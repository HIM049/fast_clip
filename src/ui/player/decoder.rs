use std::{
    collections::VecDeque,
    ffi::c_void,
    path::PathBuf,
    ptr,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::Duration,
};

use anyhow::anyhow;
use ffmpeg_next::{
    decoder::{self},
    ffi::{
        av_codec_is_decoder, av_codec_iterate, av_frame_copy_props, av_frame_unref,
        av_hwdevice_ctx_create, av_hwframe_transfer_data, avcodec_get_hw_config, AVCodecContext,
        AVHWDeviceType, AVPixelFormat, AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX,
    },
    format::{self, context, sample::Type},
    frame::{Audio, Video},
    software::{
        resampling,
        scaling::{self},
    },
    ChannelLayout, Codec, Error, Packet, Rational,
};
use gpui::{Context, Entity, SharedString};
use ringbuf::{
    traits::{Observer, Producer},
    HeapProd,
};

use crate::{
    config::{AppConfig, GpuPolicy},
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

fn output_channel_layout(channels: u16) -> ChannelLayout {
    ChannelLayout::default(i32::from(channels))
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

fn hardware_priority(policy: GpuPolicy, device_type: AVHWDeviceType) -> u8 {
    match policy {
        GpuPolicy::SoftwareOnly => u8::MAX,
        GpuPolicy::PreferIntegrated => match device_type {
            AVHWDeviceType::AV_HWDEVICE_TYPE_QSV => 0,
            AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA => 1,
            AVHWDeviceType::AV_HWDEVICE_TYPE_DXVA2 => 2,
            AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA => 3,
            _ => 4,
        },
        GpuPolicy::PreferDiscrete => match device_type {
            AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA => 0,
            AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA => 1,
            AVHWDeviceType::AV_HWDEVICE_TYPE_DXVA2 => 2,
            AVHWDeviceType::AV_HWDEVICE_TYPE_QSV => 3,
            _ => 4,
        },
    }
}

fn hardware_enabled(policy: GpuPolicy) -> bool {
    !matches!(policy, GpuPolicy::SoftwareOnly)
}

fn decoder_implementation_priority(codec: Codec) -> u8 {
    let name = codec.name();
    if name.ends_with("_cuvid") || name.ends_with("_nvdec") {
        0
    } else {
        1
    }
}

fn find_hardware_decoders(
    codec_id: ffmpeg_next::codec::Id,
    policy: GpuPolicy,
) -> Vec<(Codec, HwSelection)> {
    let mut opaque = ptr::null_mut();
    let mut candidates: Vec<(Codec, HwSelection)> = Vec::new();

    loop {
        let codec = unsafe { av_codec_iterate(&mut opaque) };
        if codec.is_null() {
            candidates.sort_by_key(|(codec, selection)| {
                (
                    hardware_priority(policy, selection.device_type),
                    decoder_implementation_priority(*codec),
                )
            });
            println!(
                "[DEBUG-hwprobe] policy={policy:?}, found {} hardware candidate(s) for {codec_id:?}",
                candidates.len(),
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
    policy: GpuPolicy,
) -> anyhow::Result<(decoder::Video, Option<Box<HwSelection>>)> {
    let codec_id = parameters.id();

    if hardware_enabled(policy) {
        for (codec, selection) in find_hardware_decoders(codec_id, policy) {
            if let Some((decoder, selection)) =
                try_open_hardware_decoder(&parameters, codec, selection)
            {
                println!(
                    "[DEBUG-hwprobe] policy={policy:?}, selected decoder={}, device={:?}, pixel_format={:?}",
                    codec.name(),
                    selection.device_type,
                    selection.pixel_format
                );
                return Ok((decoder, Some(selection)));
            }
        }
    } else {
        println!("[DEBUG-hwprobe] policy={policy:?}, hardware decoding disabled");
    }

    println!("[DEBUG-hwprobe] policy={policy:?}, using software decoder");
    Ok((open_software_video_decoder(parameters)?, None))
}

fn open_software_video_decoder(
    parameters: ffmpeg_next::codec::Parameters,
) -> anyhow::Result<decoder::Video> {
    let software_codec =
        decoder::find(parameters.id()).ok_or(anyhow!("cannot find video decoder"))?;
    let context = ffmpeg_next::codec::context::Context::from_parameters(parameters)?;
    Ok(context.decoder().open_as(software_codec)?.video()?)
}

fn open_audio_decoder(
    parameters: ffmpeg_next::codec::Parameters,
) -> anyhow::Result<decoder::Audio> {
    Ok(
        ffmpeg_next::codec::context::Context::from_parameters(parameters)?
            .decoder()
            .audio()?,
    )
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecodeMode {
    Software = 0,
    Hardware = 1,
}

impl DecodeMode {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

enum VideoDecodeResult {
    Frame(FrameImage),
    NoFrame,
    HardwareStartupFailed(Error),
    HardwareDownloadFailed(i32),
}

enum HardwareFailure {
    Startup(Error),
    Download(i32),
}

pub struct VideoDecoder {
    path: PathBuf,
    input: Option<context::Input>,
    video_stream_ix: usize,
    audio_stream_ix: usize,
    video_parameters: ffmpeg_next::codec::Parameters,
    audio_parameters: ffmpeg_next::codec::Parameters,
    v_decoder: Option<decoder::Video>,
    hw_selection: Option<Box<HwSelection>>,
    a_decoder: Option<decoder::Audio>,
    time_base: Rational,
    audio_time_base: Rational,
    duration: i64,
    device_sample_rate: u32,
    device_channels: u16,

    output_prarms: Entity<OutputParams>,
    v_producer: Option<HeapProd<FrameImage>>,
    a_producer: Option<HeapProd<f32>>,
    // size: Entity<PlayerSize>,
    // output_prarms: Entity<OutputParams>,
    event: Arc<Mutex<DecoderEvent>>,
    condvar: Arc<Condvar>,
    decode_mode: Arc<AtomicU8>,
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
        if !self.duration.is_positive() {
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
        output_channels: u16,
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

        let video_parameters = v_stream.parameters();
        let audio_parameters = a_stream.parameters();
        let gpu_policy = cx.global::<AppConfig>().gpu_policy;
        let (v_decoder, hw_selection) = open_video_decoder(video_parameters.clone(), gpu_policy)?;

        let a_decoder = open_audio_decoder(audio_parameters.clone())?;

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

        let decode_mode = if hw_selection.is_some() {
            DecodeMode::Hardware
        } else {
            DecodeMode::Software
        };

        Ok(Self {
            path: path.clone(),
            video_stream_ix: v_stream.index(),
            audio_stream_ix: a_stream.index(),
            video_parameters,
            audio_parameters,
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
            device_channels: output_channels,

            event: Arc::new(Mutex::new(DecoderEvent::None)),
            condvar: Arc::new(Condvar::new()),
            decode_mode: Arc::new(AtomicU8::new(decode_mode.as_u8())),
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

    fn resampler_params_for(a_decoder: &decoder::Audio, target_rate: u32) -> ResamplerParams {
        ResamplerParams {
            format: a_decoder.format(),
            source_rate: a_decoder.rate(),
            target_format: format::Sample::F32(Type::Packed),
            target_rate,
        }
    }

    fn create_resampler(
        source_channel_layout: ChannelLayout,
        target_channel_layout: ChannelLayout,
        params: &ResamplerParams,
    ) -> anyhow::Result<resampling::context::Context> {
        Ok(resampling::context::Context::get(
            params.format,
            source_channel_layout,
            params.source_rate,
            params.target_format,
            target_channel_layout,
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
        let mut hw_selection = self.hw_selection.take();
        let mut hardware_pixel_format = hw_selection
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
        let device_channels = self.device_channels;

        let original_size = size.read(cx).original_size();

        let video_ix = self.video_stream_ix;
        let audio_ix = self.audio_stream_ix;

        let w = v_decoder.width();
        let h = v_decoder.height();
        let event = self.event.clone();
        let condvar = self.condvar.clone();
        let path = self.path.clone();
        let video_parameters = self.video_parameters.clone();
        let audio_parameters = self.audio_parameters.clone();
        let decode_mode = self.decode_mode.clone();

        thread::spawn(move || {
            let device_channel_layout = output_channel_layout(device_channels);
            let mut scaler = None;
            let mut resampler_params = resampler_params;
            let mut w = w;
            let mut h = h;

            let mut resampler = Self::create_resampler(
                a_decoder.channel_layout(),
                device_channel_layout,
                &resampler_params,
            )
            .unwrap();

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
            let mut first_video_frame_pushed = false;

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
                        resampler = Self::create_resampler(
                            a_decoder.channel_layout(),
                            device_channel_layout,
                            &resampler_params,
                        )
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

                let mut hardware_failure = None;

                // drop extra frames when seek
                if let Some(to) = seeking_to {
                    let target =
                        (to * time_base.denominator() as f64 / time_base.numerator() as f64) as i64;
                    let audio_target = (to * audio_time_base.denominator() as f64
                        / audio_time_base.numerator() as f64)
                        as i64;
                    if !seek_state.0 {
                        match handle_video(
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
                        ) {
                            VideoDecodeResult::Frame(frame) => {
                                next_video_frame = Some(frame);
                                seek_state.0 = true;
                            }
                            VideoDecodeResult::HardwareStartupFailed(error) => {
                                hardware_failure = Some(HardwareFailure::Startup(error));
                            }
                            VideoDecodeResult::HardwareDownloadFailed(code) => {
                                hardware_failure = Some(HardwareFailure::Download(code));
                            }
                            VideoDecodeResult::NoFrame => {}
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
                        match handle_video(
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
                        ) {
                            VideoDecodeResult::Frame(frame) => next_video_frame = Some(frame),
                            VideoDecodeResult::HardwareStartupFailed(error) => {
                                hardware_failure = Some(HardwareFailure::Startup(error));
                            }
                            VideoDecodeResult::HardwareDownloadFailed(code) => {
                                hardware_failure = Some(HardwareFailure::Download(code));
                            }
                            VideoDecodeResult::NoFrame => {}
                        }
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

                if let Some(failure) = hardware_failure {
                    let switched_to_software = !first_video_frame_pushed
                        && decode_mode
                            .compare_exchange(
                                DecodeMode::Hardware.as_u8(),
                                DecodeMode::Software.as_u8(),
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            )
                            .is_ok();

                    if !switched_to_software {
                        match failure {
                            HardwareFailure::Startup(error) => {
                                eprintln!("video hardware decoder startup failed: {error}");
                            }
                            HardwareFailure::Download(code) => {
                                eprintln!("video hardware frame download failed ({code})");
                            }
                        }
                    } else {
                        match failure {
                            HardwareFailure::Startup(error) => {
                                eprintln!(
                                    "video hardware decoder startup failed: {error}; falling back to software decoder"
                                );
                            }
                            HardwareFailure::Download(code) => {
                                eprintln!(
                                    "video hardware frame download failed ({code}); falling back to software decoder"
                                );
                            }
                        }

                        input = match ffmpeg_next::format::input(&path) {
                            Ok(input) => input,
                            Err(error) => {
                                eprintln!(
                                    "video software fallback failed to reopen input: {error}"
                                );
                                break;
                            }
                        };
                        v_decoder = match open_software_video_decoder(video_parameters.clone()) {
                            Ok(decoder) => decoder,
                            Err(error) => {
                                eprintln!(
                                    "video software fallback failed to open decoder: {error}"
                                );
                                break;
                            }
                        };
                        a_decoder = match open_audio_decoder(audio_parameters.clone()) {
                            Ok(decoder) => decoder,
                            Err(error) => {
                                eprintln!(
                                    "video software fallback failed to open audio decoder: {error}"
                                );
                                break;
                            }
                        };
                        resampler_params =
                            Self::resampler_params_for(&a_decoder, resampler_params.target_rate);
                        resampler = match Self::create_resampler(
                            a_decoder.channel_layout(),
                            device_channel_layout,
                            &resampler_params,
                        ) {
                            Ok(resampler) => resampler,
                            Err(error) => {
                                eprintln!(
                                    "video software fallback failed to create resampler: {error}"
                                );
                                break;
                            }
                        };

                        hw_selection = None;
                        hardware_pixel_format = None;
                        w = v_decoder.width();
                        h = v_decoder.height();
                        scaler = None;
                        next_video_frame = None;
                        next_audio_sample = None;
                        video_pkt_queue.clear();
                        audio_pkt_queue.clear();
                        decoded_frame = Video::empty();
                        hardware_frame = Video::empty();
                        scaled_frame = Video::new(format::Pixel::BGRA, w, h);
                        decoded_audio = Audio::empty();
                        resampled_audio = Audio::empty();
                        seek_state = (false, false);
                        is_read_finished = false;
                        unsafe {
                            v_producer.set_write_index(v_producer.read_index());
                            a_producer.set_write_index(a_producer.read_index());
                        }
                        println!("DEBUG: video decoder: software fallback initialized");
                        continue;
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
                    match v_producer.try_push(f) {
                        Ok(()) => first_video_frame_pushed = true,
                        Err(f) => next_video_frame = Some(f),
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
) -> VideoDecodeResult {
    let mut reseeked = false;
    if let Some(p) = queue.pop_front() {
        match decoder.send_packet(&p) {
            Ok(()) => {}
            Err(Error::Other { errno }) if errno == ffmpeg_next::error::EAGAIN => {
                queue.push_front(p);
            }
            Err(error) if hardware_pixel_format.is_some() => {
                queue.push_front(p);
                return VideoDecodeResult::HardwareStartupFailed(error);
            }
            Err(_) => {
                queue.push_front(p);
            }
        }

        let received_frame: &mut Video = if hardware_pixel_format.is_some() {
            &mut *hardware_frame
        } else {
            &mut *decoded_frame
        };

        let received_frame = match decoder.receive_frame(received_frame) {
            Ok(()) => true,
            Err(Error::Other { errno }) if errno == ffmpeg_next::error::EAGAIN => false,
            Err(error) if hardware_pixel_format.is_some() => {
                return VideoDecodeResult::HardwareStartupFailed(error);
            }
            Err(_) => false,
        };

        if received_frame {
            if let Some(expected_pixel_format) = hardware_pixel_format {
                if unsafe { (*hardware_frame.as_ptr()).format } != expected_pixel_format as i32 {
                    eprintln!("video decoder received an unexpected software frame");
                    return VideoDecodeResult::NoFrame;
                }

                unsafe {
                    av_frame_unref(decoded_frame.as_mut_ptr());
                }
                let result = unsafe {
                    av_hwframe_transfer_data(decoded_frame.as_mut_ptr(), hardware_frame.as_ptr(), 0)
                };
                if result < 0 {
                    return VideoDecodeResult::HardwareDownloadFailed(result);
                }

                let result = unsafe {
                    av_frame_copy_props(decoded_frame.as_mut_ptr(), hardware_frame.as_ptr())
                };
                if result < 0 {
                    eprintln!("video hardware frame property copy failed ({result})");
                    return VideoDecodeResult::NoFrame;
                }
            }

            if let Some(to) = seek_to {
                if decoded_frame.pts().unwrap_or(0) < to {
                    return VideoDecodeResult::NoFrame;
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
                return VideoDecodeResult::NoFrame;
            }
            return scale_frame(
                scaled_frame,
                w,
                h,
                original_size,
                decoded_frame.pts().unwrap_or(0),
                reseeked,
            )
            .map_or(VideoDecodeResult::NoFrame, VideoDecodeResult::Frame);
        }
    }
    VideoDecodeResult::NoFrame
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_channel_layout_matches_device_channel_count() {
        assert_eq!(output_channel_layout(1).channels(), 1);
        assert_eq!(output_channel_layout(2).channels(), 2);
        assert_eq!(output_channel_layout(6).channels(), 6);
    }

    #[test]
    fn gpu_policy_changes_hardware_candidate_priority() {
        assert!(hardware_enabled(GpuPolicy::PreferIntegrated));
        assert!(hardware_enabled(GpuPolicy::PreferDiscrete));
        assert!(!hardware_enabled(GpuPolicy::SoftwareOnly));

        assert!(
            hardware_priority(
                GpuPolicy::PreferIntegrated,
                AVHWDeviceType::AV_HWDEVICE_TYPE_QSV,
            ) < hardware_priority(
                GpuPolicy::PreferIntegrated,
                AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA,
            )
        );
        assert!(
            hardware_priority(
                GpuPolicy::PreferDiscrete,
                AVHWDeviceType::AV_HWDEVICE_TYPE_CUDA,
            ) < hardware_priority(
                GpuPolicy::PreferDiscrete,
                AVHWDeviceType::AV_HWDEVICE_TYPE_QSV,
            )
        );
    }
}
