use std::{path::PathBuf, sync::Arc, thread, time::Duration};

use anyhow::anyhow;
use async_channel::{Receiver, Sender};
use ffmpeg_next::{
    Packet,
    codec::{self},
    decoder,
    format::{self, context, stream},
    packet::packet,
    software::scaling::{self},
};
use gpui::{App, AppContext, Context, Entity};

use crate::ui::{app::MyApp, player_size::PlayerSize};

pub struct VideoDecoder {
    input: Option<context::Input>,
    video_stream_ix: usize,
    audio_stream_ix: usize,
    decoder: Option<decoder::Video>,
    code_parms: codec::Parameters,

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

            size: size_entity,
        }
    }

    pub fn open(&mut self, cx: &mut Context<MyApp>, path: PathBuf) -> anyhow::Result<()> {
        let i = ffmpeg_next::format::input(&path)?;

        // for stream in i.streams() {
        //     println!(
        //         "DEBUG: Stream #{} type: {:?} rate: {} is video: {}",
        //         stream.index(),
        //         stream.parameters().medium(), // 查看是 Video, Audio 还是 Unknown
        //         stream.rate(),
        //         stream.parameters().medium() == ffmpeg_next::media::Type::Video
        //     );
        // }

        let stream = i
            .streams()
            .find(|s| s.parameters().medium() == ffmpeg_next::media::Type::Video)
            .ok_or(anyhow!("failed to find video stream"))?;

        // let stream = i
        //     .streams()
        //     .best(ffmpeg_next::media::Type::Video)
        //     .ok_or(anyhow!("failed to find video stream"))?;

        let audio = i
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .ok_or(anyhow!("failed to find video stream"))?;

        let decoder = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .video()?;

        let parmters = stream.parameters();

        let frame_rate = stream.avg_frame_rate();
        let frames = stream.frames();

        let orignal_width = decoder.width();
        let orignal_height = decoder.height();

        println!("DEBUG: frame rate: {}, frames: {}", frame_rate, frames);

        self.video_stream_ix = stream.index();
        self.audio_stream_ix = audio.index();
        self.input = Some(i);
        self.decoder = Some(decoder);
        self.code_parms = parmters;

        self.size.update(cx, |p, _| {
            p.set_orignal((orignal_width, orignal_height));
        });

        Ok(())
    }

    pub fn run(&mut self) -> Receiver<Arc<Vec<u8>>> {
        let (v_tx, rx) = async_channel::bounded::<Arc<Vec<u8>>>(60);

        Self::reader(self, v_tx);
        rx
    }

    pub fn reader(&mut self, tx: Sender<Arc<Vec<u8>>>) {
        let Some(mut input) = self.input.take() else {
            return;
        };
        let Some(mut decoder) = self.decoder.take() else {
            return;
        };
        let video_ix = self.video_stream_ix;
        let audio_ix = self.audio_stream_ix;
        let codec_parms = self.code_parms.clone();

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

            loop {
                let mut buffer = Vec::with_capacity((w * h * 4) as usize);

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
                    if tx.send_blocking(Arc::new(buffer)).is_ok() {
                        thread::sleep(Duration::from_millis(10));
                    } else {
                        break;
                    }
                }
            }
        });
    }
}
