use std::{
    collections::VecDeque,
    ffi::c_void,
    path::PathBuf,
    ptr,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use anyhow::anyhow;
use ffmpeg_next::{
    ChannelLayout, Codec, Packet, Rational,
    decoder::{self},
    ffi::{
        AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX, AVCodecContext, AVHWDeviceType, AVPixelFormat,
        av_codec_is_decoder, av_codec_iterate, av_frame_copy_props, av_frame_unref,
        av_hwdevice_ctx_create, av_hwframe_transfer_data, avcodec_get_hw_config,
    },
    format::{self, context, sample::Type},
    frame::{Audio, Video},
    software::{
        resampling,
        scaling::{self},
    },
};
use gpui::{Context, Entity, SharedString};
use ringbuf::{
    HeapProd,
    traits::{Observer, Producer},
};

use crate::{
    models::model::OutputParams,
    ui::{
        player::{
            model::{AudioRail, FrameImage},
            size::PlayerSize,
            utils::generate_image_fallback,
        },
        views::app::MyApp,
    },
};

#[derive(Debug)]
pub enum DecoderEvent {
    None,
    Stop,
    Pause,
    Seek(f64),
    LastKey(f64),
    NextKey(f64),
}

#[derive(Debug)]
pub struct ResamplerParams {
    format: format::Sample,
    source_rate: u32,
    target_format: format::Sample,
    target_rate: u32,
}

#[derive(Debug, Clone, Copy)]
struct HwSelection {
    device_type: AVHWDeviceType,
    pixel_format: AVPixelFormat,
}

unsafe extern "C" fn choose_hardware_format(
    context: *mut AVCodecContext,
    formats: *const AVPixelFormat,
) -> AVPixelFormat {
    let selection = unsafe { ((*context).opaque as *const HwSelection).as_ref() };
    let Some(selection) = selection else {
        return AVPixelFormat::AV_PIX_FMT_NONE;
    };

    let mut current = formats;
    while unsafe { *current } != AVPixelFormat::AV_PIX_FMT_NONE {
        if unsafe { *current } == selection.pixel_format {
            return selection.pixel_format;
        }
        current = unsafe { current.add(1) };
    }

    AVPixelFormat::AV_PIX_FMT_NONE
}

fn hardware_configurations(codec: Codec) -> Vec<HwSelection> {
    println!("[DEBUG-hwprobe] codec={}", codec.name());
    let codec = unsafe { codec.as_ptr() };
    let mut selections = Vec::new();
    let mut index = 0;

    loop {
        let config = unsafe { avcodec_get_hw_config(codec, index) };
        if config.is_null() {
            println!("[DEBUG-hwprobe] no more hardware configurations");
            return selections;
        }

        let config = unsafe { &*config };
        let supports_device_context =
            config.methods & AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32 != 0;

        println!(
            "[DEBUG-hwprobe] config index={index}, device={:?}, pixel_format={:?}, methods={:#x}, hw_device_ctx={supports_device_context}",
            config.device_type, config.pix_fmt, config.methods
        );

        if supports_device_context {
            selections.push(HwSelection {
                device_type: config.device_type,
                pixel_format: config.pix_fmt,
            });
        }

        index += 1;
    }
}

fn hardware_priority(device_type: AVHWDeviceType) -> u8 {
    match device_type {
        AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA => 0,
        AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA => 1,
        AVHWDeviceType::AV_HWDEVICE_TYPE_QSV => 2,
        AVHWDeviceType::AV_HWDEVICE_TYPE_DXVA2 => 3,
        _ => 4,
    }
}

fn decoder_implementation_priority(codec: Codec) -> u8 {
    let name = codec.name();
    if name.ends_with("_cuvid") || name.ends_with("_nvdec") {
        0
    } else {
        1
    }
}

fn find_hardware_decoders(codec_id: ffmpeg_next::codec::Id) -> Vec<(Codec, HwSelection)> {
    let mut opaque = ptr::null_mut();
    let mut candidates: Vec<(Codec, HwSelection)> = Vec::new();

    loop {
        let codec = unsafe { av_codec_iterate(&mut opaque) };
        if codec.is_null() {
            candidates.sort_by_key(|(codec, selection)| {
                (
                    hardware_priority(selection.device_type),
                    decoder_implementation_priority(*codec),
                )
            });
            println!(
                "[DEBUG-hwprobe] found {} hardware candidate(s) for {codec_id:?}",
                candidates.len()
            );
            return candidates;
        }

        if unsafe { av_codec_is_decoder(codec) } == 0 {
            continue;
        }

        let codec = unsafe { Codec::wrap(codec) };
        if codec.id() != codec_id {
            continue;
        }

        println!(
            "[DEBUG-hwprobe] checking decoder implementation: {}",
            codec.name()
        );
        for selection in hardware_configurations(codec) {
            candidates.push((codec, selection));
        }
    }
}

fn try_open_hardware_decoder(
    parameters: &ffmpeg_next::codec::Parameters,
    codec: Codec,
    selection: HwSelection,
) -> Option<(decoder::Video, Box<HwSelection>)> {
    println!(
        "[DEBUG-hwprobe] trying decoder={}, device={:?}, pixel_format={:?}",
        codec.name(),
        selection.device_type,
        selection.pixel_format
    );
    let mut selection = Box::new(selection);
    let mut context =
        ffmpeg_next::codec::context::Context::from_parameters(parameters.clone()).ok()?;
    let context_ptr = unsafe { context.as_mut_ptr() };
    let mut device_context = ptr::null_mut();
    let result = unsafe {
        av_hwdevice_ctx_create(
            &mut device_context,
            selection.device_type,
            ptr::null(),
            ptr::null_mut(),
            0,
        )
    };

    if result < 0 {
        println!(
            "[DEBUG-hwprobe] device creation failed for {:?}: {result}",
            selection.device_type
        );
        return None;
    }

    unsafe {
        (*context_ptr).hw_device_ctx = device_context;
        (*context_ptr).opaque = (&mut *selection as *mut HwSelection).cast::<c_void>();
        (*context_ptr).get_format = Some(choose_hardware_format);
    }

    match context
        .decoder()
        .open_as(codec)
        .and_then(|opened| opened.video())
    {
        Ok(decoder) => Some((decoder, selection)),
        Err(error) => {
            println!("[DEBUG-hwprobe] hardware open failed: {error}");
            None
        }
    }
}

fn open_video_decoder(
    parameters: ffmpeg_next::codec::Parameters,
) -> anyhow::Result<(decoder::Video, Option<Box<HwSelection>>)> {
    let codec_id = parameters.id();
    let software_codec = decoder::find(codec_id).ok_or(anyhow!("cannot find video decoder"))?;

    for (codec, selection) in find_hardware_decoders(codec_id) {
        if let Some((decoder, selection)) = try_open_hardware_decoder(&parameters, codec, selection)
        {
            println!(
                "[DEBUG-hwprobe] selected decoder={}, device={:?}, pixel_format={:?}",
                codec.name(),
                selection.device_type,
                selection.pixel_format
            );
            return Ok((decoder, Some(selection)));
        }
    }

    println!("[DEBUG-hwprobe] no hardware decoder selected; using software decoder");
    let context = ffmpeg_next::codec::context::Context::from_parameters(parameters)?;
    Ok((context.decoder().open_as(software_codec)?.video()?, None))
}

pub struct VideoDecoder {
    input: Option<context::Input>,
    video_stream_ix: usize,
    audio_stream_ix: usize,
    v_decoder: Option<decoder::Video>,
    hw_selection: Option<Box<HwSelection>>,
    a_decoder: Option<decoder::Audio>,
    time_base: Rational,
    audio_time_base: Rational,
    duration: i64,
    device_sample_rate: u32,

    output_prarms: Entity<OutputParams>,
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
        if self.duration.is_negative() {
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
            .find(|s| s.parameters().medium() == ffmpeg_next::media::Type::Audio)
            // .best(ffmpeg_next::media::Type::Audio)
            .ok_or(anyhow!("failed to find video stream"))?;

        let mut rails: Vec<AudioRail> = vec![];
        for (i, s) in i.streams().into_iter().enumerate() {
            if s.index() == v_stream.index() {
                continue;
            }
            let handler_name = match s.metadata().get("handler_name") {
                Some(s) => Some(SharedString::from(s.to_string())),
                None => None,
            };
            rails.push(AudioRail {
                code: i,
                ix: s.index(),
                id: s.id() as usize,
                duration: s.duration(),
                handler_name,
            });
        }

        let (v_decoder, hw_selection) = open_video_decoder(v_stream.parameters())?;

        // Legacy vendor-suffixed decoder selection is intentionally disabled.
        // It conflicts with the generic decoder + D3D11VA device-context path above.
        // let d = ffmpeg_next::codec::context::Context::from_parameters(v_stream.parameters())?.decoder();
        // let v_decoder = if let Some(codec) = find_best_codec(v_stream.parameters().id()) {
        //     d.open_as(codec)?.video()?
        // } else {
        //     d.video()?
        // };

        let a_decoder =
            ffmpeg_next::codec::context::Context::from_parameters(a_stream.parameters())?
                .decoder()
                .audio()?;

        let time_base = v_stream.time_base();
        let audio_time_base = a_stream.time_base();
        let duration = i.duration();
        // get original video size
        let original_width = v_decoder.width();
        let original_height = v_decoder.height();

        size.update(cx, |s, _| {
            s.set_original((original_width, original_height));
        });

        // update related output params
        output_prarms.update(cx, |p, _| {
            p.path = Some(path.clone());
            p.video_stream_ix = Some(v_stream.index());
            p.audio_stream_ix = Some(a_stream.index());
            p.audio_rails = Some(rails);
        });

        Ok(Self {
            video_stream_ix: v_stream.index(),
            audio_stream_ix: a_stream.index(),
            v_decoder: Some(v_decoder),
            hw_selection,
            a_decoder: Some(a_decoder),
            time_base,
            audio_time_base,
            duration,
            v_producer: None,
            a_producer: None,
            input: Some(i),

            output_prarms,
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
    /// TODO: let decoder take VideoDecoder struct, use only handle to control
    pub fn spawn_decoder(
        &mut self,
        size: Entity<PlayerSize>,
        cx: &mut Context<MyApp>,
        audio_ix: Option<usize>,
    ) {
        let resampler_params = self.resampler_params().unwrap();

        let Some(mut input) = self.input.take() else {
            return;
        };
        let hw_selection = self.hw_selection.take();
        let hardware_pixel_format = hw_selection
            .as_ref()
            .map(|selection| selection.pixel_format);

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
        if let Some(ix) = audio_ix {
            self.audio_stream_ix = ix;
            self.output_prarms.update(cx, |p, _| {
                p.audio_stream_ix = Some(ix);
            });
        }

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
            let mut scaler = None;

            let mut resampler =
                Self::create_resampler(a_decoder.channel_layout(), &resampler_params).unwrap();

            // frame buffer
            let mut next_video_frame: Option<FrameImage> = None;
            let mut next_audio_sample: Option<Vec<f32>> = None;

            let mut video_pkt_queue: VecDeque<Packet> = VecDeque::new();
            let mut audio_pkt_queue: VecDeque<Packet> = VecDeque::new();
            // frame varible
            let mut decoded_frame = ffmpeg_next::frame::Video::empty();
            let mut hardware_frame = ffmpeg_next::frame::Video::empty();
            let mut scaled_frame = ffmpeg_next::frame::Video::new(format::Pixel::BGRA, w, h);
            let mut decoded_audio = ffmpeg_next::frame::Audio::empty();
            let mut resampled_audio = ffmpeg_next::frame::Audio::empty();

            let mut seeking_to: Option<f64> = None;
            let mut seek_state = (false, false);
            let mut is_read_finished = false;

            loop {
                {
                    let mut need_flash = false;
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
                            let ts = (ffmpeg_next::sys::AV_TIME_BASE as f64 * t) as i64;
                            if let Err(e) = input.seek(ts, ..ts) {
                                eprintln!("video seek failed: {e}");
                                continue;
                            }

                            is_read_finished = false;
                            seeking_to = Some(t);
                            seek_state = (false, false);
                            need_flash = true;
                        }
                        DecoderEvent::LastKey(t) => {
                            let ts = (ffmpeg_next::sys::AV_TIME_BASE as f64 * t) as i64;
                            if let Err(e) = input.seek(ts, ..ts) {
                                eprintln!("video seek failed: {e}");
                                continue;
                            }

                            is_read_finished = false;
                            seeking_to = Some(t);
                            seek_state = (false, false);
                            need_flash = true;
                        }
                        DecoderEvent::NextKey(t) => {
                            let ts = (ffmpeg_next::sys::AV_TIME_BASE as f64 * t) as i64;
                            if let Err(e) = input.seek(ts, ts..) {
                                eprintln!("video seek failed: {e}");
                                continue;
                            }

                            is_read_finished = false;
                            seeking_to = Some(t);
                            seek_state = (false, false);
                            need_flash = true;
                        }
                    }
                    if need_flash {
                        v_decoder.flush();
                        a_decoder.flush();
                        video_pkt_queue.clear();
                        audio_pkt_queue.clear();

                        // create new resampler
                        resampler =
                            Self::create_resampler(a_decoder.channel_layout(), &resampler_params)
                                .unwrap();

                        unsafe {
                            a_producer.set_write_index(a_producer.read_index());
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

                // drop extra frames when seek
                if let Some(to) = seeking_to {
                    let target =
                        (to * time_base.denominator() as f64 / time_base.numerator() as f64) as i64;
                    let audio_target = (to * audio_time_base.denominator() as f64
                        / audio_time_base.numerator() as f64)
                        as i64;
                    if !seek_state.0 {
                        let result = handle_video(
                            &mut video_pkt_queue,
                            &mut v_decoder,
                            &mut decoded_frame,
                            &mut hardware_frame,
                            &mut scaler,
                            &mut scaled_frame,
                            w,
                            h,
                            original_size,
                            Some(target),
                            hardware_pixel_format,
                        );
                        if result.is_some() {
                            next_video_frame = result;
                            seek_state.0 = true;
                        }
                    }
                    if !seek_state.1 {
                        let result = handle_audio(
                            &mut audio_pkt_queue,
                            &mut a_decoder,
                            &mut resampler,
                            &mut decoded_audio,
                            &mut resampled_audio,
                            Some(audio_target),
                        );
                        if result.is_some() {
                            next_audio_sample = result;
                            seek_state.1 = true;
                        }
                    }
                    if seek_state == (true, true) {
                        seeking_to = None;
                    }
                } else {
                    if next_video_frame.is_none() {
                        next_video_frame = handle_video(
                            &mut video_pkt_queue,
                            &mut v_decoder,
                            &mut decoded_frame,
                            &mut hardware_frame,
                            &mut scaler,
                            &mut scaled_frame,
                            w,
                            h,
                            original_size,
                            None,
                            hardware_pixel_format,
                        );
                    }
                    if next_audio_sample.is_none() {
                        next_audio_sample = handle_audio(
                            &mut audio_pkt_queue,
                            &mut a_decoder,
                            &mut resampler,
                            &mut decoded_audio,
                            &mut resampled_audio,
                            None,
                        );
                    }
                }

                // if ringbuf is full
                if v_producer.is_full() && a_producer.is_full()
                    || is_read_finished && next_video_frame.is_none() && next_audio_sample.is_none()
                {
                    thread::sleep(Duration::from_millis(10));
                }

                // push frame to ringbuf
                if let Some(f) = next_video_frame.take() {
                    if let Err(f) = v_producer.try_push(f) {
                        next_video_frame = Some(f);
                    }
                }
                // push audio sample to ringbuf
                if let Some(s) = next_audio_sample.take() {
                    let written = a_producer.push_slice(&s);
                    if written < s.len() {
                        next_audio_sample = Some(s[written..].to_vec())
                    }
                }
            }

            drop(v_decoder);
            drop(hw_selection);
        });
    }
}

fn handle_video(
    queue: &mut VecDeque<Packet>,
    decoder: &mut decoder::Video,
    decoded_frame: &mut Video,
    hardware_frame: &mut Video,
    scaler: &mut Option<ffmpeg_next::software::scaling::Context>,
    scaled_frame: &mut ffmpeg_next::frame::Video,
    w: u32,
    h: u32,
    original_size: (u32, u32),
    seek_to: Option<i64>,
    hardware_pixel_format: Option<AVPixelFormat>,
) -> Option<FrameImage> {
    let mut reseeked = false;
    if let Some(p) = queue.pop_front() {
        if decoder.send_packet(&p).is_err() {
            queue.push_front(p);
        }

        let received_frame: &mut Video = if hardware_pixel_format.is_some() {
            &mut *hardware_frame
        } else {
            &mut *decoded_frame
        };

        if decoder.receive_frame(received_frame).is_ok() {
            if let Some(expected_pixel_format) = hardware_pixel_format {
                if unsafe { (*hardware_frame.as_ptr()).format } != expected_pixel_format as i32 {
                    eprintln!("video decoder received an unexpected software frame");
                    return None;
                }

                unsafe {
                    av_frame_unref(decoded_frame.as_mut_ptr());
                }
                let result = unsafe {
                    av_hwframe_transfer_data(decoded_frame.as_mut_ptr(), hardware_frame.as_ptr(), 0)
                };
                if result < 0 {
                    eprintln!("video hardware frame download failed ({result})");
                    return None;
                }

                let result = unsafe {
                    av_frame_copy_props(decoded_frame.as_mut_ptr(), hardware_frame.as_ptr())
                };
                if result < 0 {
                    eprintln!("video hardware frame property copy failed ({result})");
                    return None;
                }
            }

            if let Some(to) = seek_to {
                if decoded_frame.pts().unwrap_or(0) < to {
                    return None;
                } else {
                    reseeked = true;
                }
            }

            let scaler = scaler.get_or_insert_with(|| {
                ffmpeg_next::software::scaling::Context::get(
                    decoded_frame.format(),
                    w,
                    h,
                    format::Pixel::BGRA,
                    w,
                    h,
                    scaling::Flags::BILINEAR,
                )
                .expect("failed to create video scaler")
            });

            if scaler.run(decoded_frame, scaled_frame).is_err() {
                return None;
            }
            return scale_frame(
                scaled_frame,
                w,
                h,
                original_size,
                decoded_frame.pts().unwrap_or(0),
                reseeked,
            );
        }
    }
    None
}

fn handle_audio(
    queue: &mut VecDeque<Packet>,
    decoder: &mut decoder::Audio,
    resampler: &mut resampling::context::Context,
    decoded_audio: &mut Audio,
    resampled_audio: &mut Audio,
    seek_to: Option<i64>,
) -> Option<Vec<f32>> {
    // push if some audio packet
    if let Some(p) = queue.pop_front() {
        if decoder.send_packet(&p).is_err() {
            queue.push_front(p);
        }
    }
    if decoder.receive_frame(decoded_audio).is_ok() {
        if let Some(to) = seek_to {
            if decoded_audio.pts().unwrap_or(0) < to {
                return None;
            }
        }
        // try receive audio frame and resample
        resampler.run(&decoded_audio, resampled_audio).unwrap();
    } else if queue.len() == 0 {
        // queue are clear, release resampler
        if let Ok(r) = resampler.flush(resampled_audio) {
            if r.is_none() {
                // break;
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
        return Some(raw_samples.to_vec());
    }
    None
}

pub fn scale_frame(
    scaled_frame: &mut ffmpeg_next::frame::Video,
    width: u32,
    height: u32,
    original_size: (u32, u32),
    pts: i64,
    reseeked: bool,
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
        reseeked,
    })
}
