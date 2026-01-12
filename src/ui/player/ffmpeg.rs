use std::{
    any::Any,
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use anyhow::anyhow;
use ffmpeg_next::{
    ChannelLayout, Codec, Packet, Rational,
    codec::Id,
    decoder::{self},
    ffi::{AVCodecHWConfig, avcodec_get_hw_config},
    format::{self, context, sample::Type},
    software::{
        resampling,
        scaling::{self},
    },
};
use gpui::{Context, Entity};
use ringbuf::{
    HeapProd,
    traits::{Observer, Producer},
};

use crate::{
    models::model::OutputParams,
    ui::{
        player::{frame::FrameImage, size::PlayerSize, utils::generate_image_fallback},
        views::app::MyApp,
    },
};

#[derive(Debug)]
pub enum DecoderEvent {
    None,
    Stop,
    Pause,
    Seek(f32),
}

#[derive(Debug)]
pub struct ResamplerParams {
    format: format::Sample,
    source_rate: u32,
    target_format: format::Sample,
    target_rate: u32,
}

pub struct VideoDecoder {
    input: Option<context::Input>,
    video_stream_ix: usize,
    audio_stream_ix: usize,
    v_decoder: Option<decoder::Video>,
    a_decoder: Option<decoder::Audio>,
    time_base: Rational,
    audio_time_base: Rational,
    duration: i64,
    device_sample_rate: u32,

    v_producer: Option<HeapProd<FrameImage>>,
    a_producer: Option<HeapProd<f32>>,
    // size: Entity<PlayerSize>,
    // output_prarms: Entity<OutputParams>,
    event: Arc<Mutex<DecoderEvent>>,
    condvar: Arc<Condvar>,
}

impl VideoDecoder {
    /// set producer of ringbuf in VideoDecoder
    pub fn set_video_producer(mut self, p: HeapProd<FrameImage>) -> Self {
        self.v_producer = Some(p);
        self
    }

    pub fn set_audio_producer(mut self, p: HeapProd<f32>) -> Self {
        self.a_producer = Some(p);
        self
    }

    /// set DecoderEvent
    pub fn set_event(&mut self, new: DecoderEvent) {
        let mut event = self.event.lock().unwrap();
        *event = new;
        self.condvar.notify_all();
    }

    /// get video timebase
    pub fn get_timebase(&self) -> Rational {
        self.time_base
    }

    pub fn get_duration(&self) -> Option<i64> {
        if self.duration == 0 {
            return None;
        }
        Some(self.duration)
    }

    /// open a video file
    pub fn open<T>(
        cx: &mut Context<T>,
        path: &PathBuf,
        size: Entity<PlayerSize>,
        output_prarms: Entity<OutputParams>,
        sample_rate: u32,
    ) -> anyhow::Result<Self>
    where
        T: 'static,
    {
        let i = ffmpeg_next::format::input(path)?;

        let v_stream = i
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or(anyhow!("failed to find best video stream"))?;

        let a_stream = i
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            // .find(|s|{ s.id() == 2})
            .ok_or(anyhow!("failed to find video stream"))?;

        let d =
            ffmpeg_next::codec::context::Context::from_parameters(v_stream.parameters())?.decoder();

        let v_decoder = if let Some(codec) = find_best_codec(v_stream.parameters().id()) {
            println!("DEBUG: useing codec: {}", codec.name());
            d.open_as(codec)?.video()?
        } else {
            d.video()?
        };

        let a_decoder =
            ffmpeg_next::codec::context::Context::from_parameters(a_stream.parameters())?
                .decoder()
                .audio()?;

        let time_base = v_stream.time_base();
        let audio_time_base = a_stream.time_base();
        // get sample rate and length of video frams
        let frame_rate = v_stream.avg_frame_rate();
        let duration = v_stream.duration();
        // get original video size
        let original_width = v_decoder.width();
        let original_height = v_decoder.height();

        println!("DEBUG: frame rate: {}, duration: {}", frame_rate, duration);

        size.update(cx, |s, _| {
            s.set_original((original_width, original_height));
        });

        // update related output params
        output_prarms.update(cx, |p, _| {
            p.path = Some(path.clone());
            p.video_stream_ix = Some(v_stream.index());
            p.audio_stream_ix = Some(a_stream.index());
        });

        Ok(Self {
            video_stream_ix: v_stream.index(),
            audio_stream_ix: a_stream.index(),
            v_decoder: Some(v_decoder),
            a_decoder: Some(a_decoder),
            time_base,
            audio_time_base,
            duration,
            v_producer: None,
            a_producer: None,
            // size,
            // output_prarms,
            input: Some(i),

            device_sample_rate: sample_rate,

            event: Arc::new(Mutex::new(DecoderEvent::None)),
            condvar: Arc::new(Condvar::new()),
        })
    }

    fn resampler_params(&self) -> anyhow::Result<ResamplerParams> {
        let Some(a_decoder) = self.a_decoder.as_ref() else {
            return Err(anyhow!("none decoder"));
        };
        Ok(ResamplerParams {
            format: a_decoder.format(),
            source_rate: a_decoder.rate(),
            target_format: format::Sample::F32(Type::Packed),
            target_rate: self.device_sample_rate,
        })
    }

    fn create_resampler(
        channel_layout: ChannelLayout,
        params: &ResamplerParams,
    ) -> anyhow::Result<resampling::context::Context> {
        Ok(resampling::context::Context::get(
            params.format,
            channel_layout,
            params.source_rate,
            params.target_format,
            channel_layout,
            params.target_rate,
        )?)
    }

    /// spawn decoder thread
    pub fn spawn_decoder(&mut self, size: Entity<PlayerSize>, cx: &mut Context<MyApp>) {
        let resampler_params = self.resampler_params().unwrap();

        let Some(mut input) = self.input.take() else {
            return;
        };
        let Some(mut v_decoder) = self.v_decoder.take() else {
            return;
        };
        let Some(mut a_decoder) = self.a_decoder.take() else {
            return;
        };
        let Some(mut v_producer) = self.v_producer.take() else {
            return;
        };
        let Some(mut a_producer) = self.a_producer.take() else {
            return;
        };

        let time_base = self.time_base;
        let audio_time_base = self.audio_time_base;

        let original_size = size.read(cx).original_size();

        let video_ix = self.video_stream_ix;
        let audio_ix = self.audio_stream_ix;

        let w = v_decoder.width();
        let h = v_decoder.height();
        let event = self.event.clone();
        let condvar = self.condvar.clone();

        thread::spawn(move || {
            // init ffmpeg scaler
            let mut scaler = ffmpeg_next::software::scaling::Context::get(
                v_decoder.format(),
                w,
                h,
                format::Pixel::BGRA,
                w,
                h,
                scaling::Flags::BILINEAR,
            )
            .unwrap();

            println!("DEBUG: audio sample rate {}", a_decoder.rate());

            let mut resampler =
                Self::create_resampler(a_decoder.channel_layout(), &resampler_params).unwrap();

            // frame buffer
            let mut next_video_frame: Option<FrameImage> = None;
            let mut next_audio_sample: Option<Vec<f32>> = None;

            let mut video_pkt_queue: VecDeque<Packet> = VecDeque::new();
            let mut audio_pkt_queue: VecDeque<Packet> = VecDeque::new();
            // frame varible
            let mut decoded_frame = ffmpeg_next::frame::Video::new(v_decoder.format(), w, h);
            let mut scaled_frame = ffmpeg_next::frame::Video::new(format::Pixel::BGRA, w, h);
            let mut decoded_audio = ffmpeg_next::frame::Audio::empty();
            let mut resampled_audio = ffmpeg_next::frame::Audio::empty();

            let mut seeking_to: Option<f32> = None;
            let mut seek_state = (false, false);
            let mut is_read_finished = false;
            loop {
                {
                    // handle decoder event
                    let mut event = event.lock().unwrap();
                    match *event {
                        DecoderEvent::None => (),
                        DecoderEvent::Stop => break,
                        DecoderEvent::Pause => {
                            let _event = condvar.wait(event).unwrap();
                            continue;
                        }
                        DecoderEvent::Seek(t) => {
                            let ts = (ffmpeg_next::sys::AV_TIME_BASE as f32 * t) as i64;
                            if let Err(e) = input.seek(ts, ..ts) {
                                println!("DEBUG: failed when seek: {}", e);
                                continue;
                            }

                            v_decoder.flush();
                            a_decoder.flush();
                            seeking_to = Some(t);
                            seek_state = (false, false);

                            // create new resampler
                            resampler = Self::create_resampler(
                                a_decoder.channel_layout(),
                                &resampler_params,
                            )
                            .unwrap();

                            video_pkt_queue.clear();
                            audio_pkt_queue.clear();

                            unsafe {
                                a_producer.set_write_index(a_producer.read_index());
                            }
                        }
                    }
                    *event = DecoderEvent::None;
                }
                // if no enough pkts, read from file
                while !is_read_finished
                    && (video_pkt_queue.len() < 50 || audio_pkt_queue.len() < 100)
                {
                    // read packets
                    if let Some((stream, packet)) = input.packets().next() {
                        if stream.index() == video_ix {
                            video_pkt_queue.push_back(packet);
                        } else if stream.index() == audio_ix {
                            audio_pkt_queue.push_back(packet);
                        }
                    } else {
                        is_read_finished = true;
                    }
                }

                // push if some video packet
                if let Some(p) = video_pkt_queue.pop_front() {
                    if v_decoder.send_packet(&p).is_err() {
                        video_pkt_queue.push_front(p);
                    }
                }

                // push if some audio packet
                if let Some(p) = audio_pkt_queue.pop_front() {
                    if a_decoder.send_packet(&p).is_err() {
                        audio_pkt_queue.push_front(p);
                    }
                }

                // drop extra frames when seek
                if let Some(to) = seeking_to {
                    let target = (to * time_base.denominator() as f32) as i64;
                    let audio_target = (to * audio_time_base.denominator() as f32) as i64;
                    if !seek_state.0 && v_decoder.receive_frame(&mut decoded_frame).is_ok() {
                        if decoded_frame.pts().unwrap_or(0) < target {
                            continue;
                        } else {
                            println!("v skip to{:?}", decoded_frame.pts());
                            scaler.run(&decoded_frame, &mut scaled_frame).unwrap();
                            next_video_frame = scale_frame(
                                &mut scaled_frame,
                                w,
                                h,
                                original_size,
                                decoded_frame.pts().unwrap_or(0),
                            );
                            seek_state.0 = true;
                        }
                    }
                    if !seek_state.1 && a_decoder.receive_frame(&mut decoded_audio).is_ok() {
                        if decoded_audio.pts().unwrap_or(0) < audio_target {
                            continue;
                        } else {
                            println!("a skip to{:?}", decoded_audio.pts());

                            resampler.run(&decoded_audio, &mut resampled_audio).unwrap();
                            if resampled_audio.samples() > 0 {
                                let raw_samples: &[f32] = unsafe {
                                    std::slice::from_raw_parts(
                                        resampled_audio.data(0).as_ptr() as *const f32,
                                        resampled_audio.samples()
                                            * resampled_audio.channels() as usize,
                                    )
                                };
                                next_audio_sample = Some(raw_samples.to_vec());
                                seek_state.1 = true;
                            }
                        }
                    }
                    if seek_state == (true, true) {
                        seeking_to = None;
                    }
                } else {
                    // try receive decoded frame and push
                    if next_video_frame.is_none()
                        && v_decoder.receive_frame(&mut decoded_frame).is_ok()
                    {
                        // convert frame
                        scaler.run(&decoded_frame, &mut scaled_frame).unwrap();
                        next_video_frame = scale_frame(
                            &mut scaled_frame,
                            w,
                            h,
                            original_size,
                            decoded_frame.pts().unwrap_or(0),
                        );
                    }

                    // if none ready audio sample
                    if next_audio_sample.is_none() {
                        if a_decoder.receive_frame(&mut decoded_audio).is_ok() {
                            // try receive audio frame and resample
                            resampler.run(&decoded_audio, &mut resampled_audio).unwrap();
                        } else if audio_pkt_queue.len() == 0 {
                            // queue are clear, release resampler
                            if let Ok(r) = resampler.flush(&mut resampled_audio) {
                                if r.is_none() {
                                    break;
                                }
                            }
                        }
                        if resampled_audio.samples() > 0 {
                            let raw_samples: &[f32] = unsafe {
                                std::slice::from_raw_parts(
                                    resampled_audio.data(0).as_ptr() as *const f32,
                                    resampled_audio.samples() * resampled_audio.channels() as usize,
                                )
                            };
                            next_audio_sample = Some(raw_samples.to_vec());
                        }
                    }
                }

                // if ringbuf is full
                if v_producer.is_full() && a_producer.is_full() {
                    thread::sleep(Duration::from_millis(10));
                } else if is_read_finished
                    && next_video_frame.is_none()
                    && next_audio_sample.is_none()
                {
                    break;
                }

                // push video frame
                if let Some(f) = next_video_frame.take() {
                    if let Err(f) = v_producer.try_push(f) {
                        next_video_frame = Some(f);
                    }
                }

                // push audio sample
                if let Some(s) = next_audio_sample.take() {
                    let written = a_producer.push_slice(&s);
                    if written < s.len() {
                        next_audio_sample = Some(s[written..].to_vec())
                    }
                }
            }
        });
    }
}

pub fn scale_frame(
    scaled_frame: &mut ffmpeg_next::frame::Video,
    width: u32,
    height: u32,
    original_size: (u32, u32),
    pts: i64,
) -> Option<FrameImage> {
    let data = scaled_frame.data(0);
    let stride = scaled_frame.stride(0);

    let mut buffer = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height as usize {
        let start = y * stride;
        let end = start + (width as usize * 4);
        buffer.extend_from_slice(&data[start..end]);
    }

    Some(FrameImage {
        image: generate_image_fallback(original_size, buffer),
        pts,
    })
}

pub fn find_best_codec(id: ffmpeg_next::codec::Id) -> Option<Codec> {
    let codec_name_base = match id {
        ffmpeg_next::codec::Id::H264 => "h264",
        ffmpeg_next::codec::Id::HEVC => "hevc",
        ffmpeg_next::codec::Id::AV1 => "av1",
        ffmpeg_next::codec::Id::VP9 => "vp9",
        ffmpeg_next::codec::Id::MJPEG => "mjpeg",
        _ => return None,
    };

    let hw_priorities = [
        "cuvid",        // Nvidia (Windows/Linux) - 性能最好，自带显存管理
        "videotoolbox", // macOS - 苹果原生，效率极高
        "qsv",          // Intel (Windows/Linux) - QuickSync
        "mediacodec",   // Android
        "rkmpp",        // Rockchip (树莓派/开发板)
    ];

    // 3. 遍历尝试
    for suffix in hw_priorities {
        let candidate_name = format!("{}_{}", codec_name_base, suffix);
        // try to find codec by name
        let codec = ffmpeg_next::decoder::find_by_name(&candidate_name);
        if codec.is_some() {
            return codec;
        }
    }

    None
}
