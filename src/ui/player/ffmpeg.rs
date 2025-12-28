use std::{path::PathBuf, thread, time::Duration};

use anyhow::anyhow;
use ffmpeg_next::{
    Rational,
    codec::{self},
    decoder,
    format::{self, context},
    software::scaling::{self},
};
use gpui::{Context, Entity};
use ringbuf::{
    HeapProd,
    traits::{Observer, Producer},
};

use crate::ui::{
    app::MyApp,
    player::{frame::FrameImage, player_size::PlayerSize, utils::generate_image_fallback},
};

pub struct VideoDecoder {
    input: Option<context::Input>,
    video_stream_ix: usize,
    audio_stream_ix: usize,
    decoder: Option<decoder::Video>,
    code_parms: codec::Parameters,
    time_base: Option<Rational>,

    producer: Option<HeapProd<FrameImage>>,
    size: Entity<PlayerSize>,
}

impl VideoDecoder {
    pub fn new(size_entity: Entity<PlayerSize>) -> Self {
        Self {
            input: None,
            video_stream_ix: 0,
            audio_stream_ix: 0,
            decoder: None,
            code_parms: codec::Parameters::new(),
            time_base: None,

            producer: None,
            size: size_entity,
        }
    }

    pub fn set_producer(mut self, p: HeapProd<FrameImage>) -> Self {
        self.producer = Some(p);
        self
    }

    pub fn get_timebase(&self) -> Option<Rational> {
        self.time_base
    }

    pub fn open(&mut self, cx: &mut Context<MyApp>, path: PathBuf) -> anyhow::Result<()> {
        let i = ffmpeg_next::format::input(&path)?;

        let stream = i
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or(anyhow!("failed to find video stream"))?;

        let audio = i
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .ok_or(anyhow!("failed to find video stream"))?;

        let decoder = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .video()?;

        let time_base = stream.time_base();

        let parmters = stream.parameters();
        // get sample rate and length of video frams
        let frame_rate = stream.avg_frame_rate();
        let frames = stream.frames();
        // get orignal video size
        let orignal_width = decoder.width();
        let orignal_height = decoder.height();

        println!("DEBUG: frame rate: {}, frames: {}", frame_rate, frames);

        self.video_stream_ix = stream.index();
        self.audio_stream_ix = audio.index();
        self.input = Some(i);
        self.decoder = Some(decoder);
        self.code_parms = parmters;
        self.time_base = Some(time_base);

        self.size.update(cx, |p, _| {
            p.set_orignal((orignal_width, orignal_height));
        });

        Ok(())
    }

    pub fn spawn_decoder(&mut self, size: Entity<PlayerSize>, cx: &mut Context<MyApp>) {
        let Some(mut input) = self.input.take() else {
            return;
        };
        let Some(mut decoder) = self.decoder.take() else {
            return;
        };
        let Some(mut producer) = self.producer.take() else {
            return;
        };
        let orignal_size = size.read(cx).orignal_size();

        let video_ix = self.video_stream_ix;
        let audio_ix = self.audio_stream_ix;

        let w = decoder.width();
        let h = decoder.height();

        let mut decoded_frame = ffmpeg_next::frame::Video::new(decoder.format(), w, h);
        let mut scaled_frame = ffmpeg_next::frame::Video::new(format::Pixel::BGRA, w, h);

        thread::spawn(move || {
            let mut scaler_context = ffmpeg_next::software::scaling::Context::get(
                decoder.format(),
                w,
                h,
                format::Pixel::BGRA,
                w,
                h,
                scaling::Flags::BILINEAR,
            )
            .unwrap();

            let mut frame_buf: Option<FrameImage> = None;

            loop {
                let mut buffer = Vec::with_capacity((w * h * 4) as usize);

                if frame_buf.is_none() {
                    // read packets
                    if let Some((stream, packet)) = input.packets().next() {
                        if stream.index() == video_ix {
                            // try to send packet to decoder
                            if decoder.send_packet(&packet).is_err() {
                                println!("DEBUG: error when send packet");
                                return;
                            }
                        } else if stream.index() == audio_ix {
                            // channel send
                        }
                    } else {
                        break;
                    };

                    // try receive decoder
                    if decoder.receive_frame(&mut decoded_frame).is_ok() {
                        scaler_context
                            .run(&decoded_frame, &mut scaled_frame)
                            .unwrap();

                        let data = scaled_frame.data(0);
                        let stride = scaled_frame.stride(0);

                        for y in 0..h as usize {
                            let start = y * stride;
                            let end = start + (w as usize * 4);
                            buffer.extend_from_slice(&data[start..end]);
                        }

                        frame_buf = Some(FrameImage {
                            image: generate_image_fallback(orignal_size, buffer),
                            pts: decoded_frame.pts().unwrap_or(0) as u64,
                        });
                    }
                }

                if producer.is_full() {
                    thread::sleep(Duration::from_millis(10));
                }
                if let Some(f) = frame_buf.take() {
                    if let Err(f) = producer.try_push(f) {
                        frame_buf = Some(f);
                    }
                }
            }
        });
    }
}
