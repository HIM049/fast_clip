use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use gpui::{Context, Entity, RenderImage, Window};
use ringbuf::{
    HeapCons, HeapProd,
    storage::Heap,
    traits::{Consumer, Split},
};

use crate::{
    models::model::OutputParams,
    ui::{
        player::{
            audio::AudioPlayer,
            ffmpeg::{DecoderEvent, VideoDecoder},
            frame::{FrameAction, FrameImage},
            size::PlayerSize,
            timer::Timer,
            utils::generate_image_fallback,
            viewer::Viewer,
        },
        views::app::MyApp,
    },
};

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

pub struct Player {
    init: bool,
    timer: Timer,
    size: Entity<PlayerSize>,
    output_params: Entity<OutputParams>,
    decoder: Option<VideoDecoder>,
    frame: Arc<RenderImage>,
    frame_buf: Option<FrameImage>,
    producer: Option<HeapProd<FrameImage>>,
    a_producer: Option<HeapProd<f32>>,
    consumer: HeapCons<FrameImage>,
    // played_time: Option<f32>,
    state: PlayState,

    audio_player: AudioPlayer,

    recent_pts: f64,
    is_time_set: bool,
    is_seeking: bool,
    is_waiting: bool,

    sample_count: Arc<AtomicUsize>,
    play_signal: Arc<AtomicBool>,
}

impl Player {
    pub fn new(size_entity: Entity<PlayerSize>, output_params: Entity<OutputParams>) -> Self {
        let rb = ringbuf::SharedRb::<Heap<FrameImage>>::new(30 * 1);
        let (v_producer, v_consumer) = rb.split();

        let mut audio_player = AudioPlayer::new().unwrap();
        let rb = ringbuf::SharedRb::<Heap<f32>>::new(audio_player.sample_rate() as usize * 1);
        let (a_producer, a_consumer) = rb.split();

        let sample_count = Arc::new(AtomicUsize::new(0));
        let play_signal = Arc::new(AtomicBool::new(false));
        audio_player.spawn(a_consumer, sample_count.clone(), play_signal.clone());

        Self {
            init: false,
            timer: Timer::new(),
            size: size_entity.clone(),
            output_params: output_params.clone(),
            decoder: None,
            frame: generate_image_fallback((1, 1), vec![]),
            frame_buf: None,
            producer: Some(v_producer),
            a_producer: Some(a_producer),
            consumer: v_consumer,
            // played_time: None,
            state: PlayState::Stopped,

            audio_player,

            recent_pts: 0.0,
            is_time_set: false,
            is_seeking: false,
            is_waiting: false,

            sample_count,
            play_signal,
        }
    }

    pub fn is_init(&self) -> bool {
        self.init
    }

    pub fn open<T>(&mut self, cx: &mut Context<T>, path: &PathBuf) -> anyhow::Result<()>
    where
        T: 'static,
    {
        let decoder = VideoDecoder::open(
            cx,
            path,
            self.size.clone(),
            self.output_params.clone(),
            self.audio_player.sample_rate(),
        );
        match decoder {
            Ok(d) => {
                let d = d
                    .set_video_producer(self.producer.take().unwrap())
                    .set_audio_producer(self.a_producer.take().unwrap());
                self.decoder = Some(d);
            }
            Err(e) => println!("error: {}", e),
        }
        self.init = true;
        Ok(())
    }

    pub fn start_play(&mut self, cx: &mut Context<MyApp>) {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.spawn_decoder(self.size.clone(), cx);
            self.state = PlayState::Playing;
            self.timer.start();
        }
    }

    // fn start_time(&mut self) {
    //     self.timer.start();
    //     self.sample_count.store(0, Ordering::Relaxed);
    //     self.audio_player.play().unwrap();
    // }

    pub fn resume_play(&mut self) {
        self.state = PlayState::Playing;
        self.timer.start();
        self.audio_player.play().unwrap();
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.set_event(DecoderEvent::None);
        }
    }

    pub fn pause_play(&mut self) {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.set_event(DecoderEvent::Pause);
            self.state = PlayState::Paused;
            self.timer.stop();
            self.audio_player.pause().unwrap();
        }
    }

    pub fn stop_play(&mut self) {
        self.state = PlayState::Stopped;
        self.frame = generate_image_fallback((1, 1), vec![]);
        self.frame_buf = None;
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.set_event(DecoderEvent::Stop);
        }
        // TODO: drop decoder
    }

    pub fn next_key(&mut self) {
        self.is_waiting = true;
        // self.pause_timer();
        self.timer.stop();
        let ct = self.timer.current_time_sec();

        if let Some(d) = self.decoder.as_mut() {
            d.set_event(DecoderEvent::NextKey(ct));
        }
    }

    pub fn set_playtime<F>(&mut self, update_fn: F)
    where
        F: Fn(f64, f64) -> f64,
    {
        self.timer.stop();
        self.is_time_set = true;
        let now = self.timer.current_time_sec();
        let dur_sec = self.duration_sec().unwrap_or(0.);
        self.timer
            .set_time_sec(update_fn(now, dur_sec).clamp(0.0, dur_sec));
    }

    pub fn get_state(&self) -> PlayState {
        self.state
    }

    pub fn play_percentage(&self) -> Option<f32> {
        let Some(decoder) = self.decoder.as_ref() else {
            return None;
        };
        let Some(duration) = decoder.get_duration() else {
            return None;
        };
        let timebase = decoder.get_timebase();
        if duration.is_negative() {
            return None;
        }
        let d_sec = (duration as f64 / timebase.denominator() as f64) as f32;
        Some((self.current_playtime() / d_sec as f64) as f32)
    }

    pub fn duration_sec(&self) -> Option<f64> {
        let Some(decoder) = self.decoder.as_ref() else {
            return None;
        };
        let Some(duration) = decoder.get_duration() else {
            return None;
        };
        let timebase = decoder.get_timebase();
        Some((duration as f64 / timebase.denominator() as f64))
    }

    // calc played samples time
    fn played_sample_sec(&self) -> f32 {
        self.sample_count.load(Ordering::Relaxed) as f32 / self.audio_player.sample_rate() as f32
    }

    // calc current time
    pub fn current_playtime(&self) -> f64 {
        self.timer.current_time_sec()
    }

    // // save current time & pause audio output
    // fn pause_timer(&mut self) {
    //     self.played_time = Some(self.current_playtime());
    //     self.audio_player.pause().unwrap();
    //     self.sample_count.store(0, Ordering::Relaxed);
    // }

    fn frame_time(&self, pts: i64) -> Option<f64> {
        if pts.is_negative() {
            return None;
        }
        let Some(decoder) = self.decoder.as_ref() else {
            return None;
        };
        let time_base = decoder.get_timebase();
        Some(pts as f64 / time_base.denominator() as f64)
    }

    /// only for playing control
    fn compare_time(&mut self, frame_pts: i64) -> FrameAction {
        let Some(frame_time) = self.frame_time(frame_pts) else {
            return FrameAction::Wait;
        };
        self.recent_pts = frame_time;

        let play_time = self.current_playtime();
        if (play_time - frame_time).abs() <= 0.3 {
            if frame_time <= play_time {
                FrameAction::Render
            } else {
                FrameAction::Wait
            }
        } else {
            if self.is_seeking && !self.is_time_set {
                return FrameAction::Drop;
            } else {
                self.is_seeking = true;
                self.is_time_set = false;
                // self.pause_timer();
                self.timer.stop();
                FrameAction::ReSeek(play_time)
            }
        }
    }

    pub fn dbg_msg(&self) -> String {
        format!(
            "
            PlayInfo: PT {:.2}, RFT {:.2}, SEEKING {}, UNHANDLE_SET {}, DIFF {:.2}
            played_sample_sec {:.2}, played_time {:?}
            ",
            self.current_playtime(),
            self.recent_pts,
            self.is_seeking,
            self.is_time_set,
            self.current_playtime() - self.recent_pts,
            self.played_sample_sec(),
            self.timer.current_time_sec()
        )
    }

    pub fn view(&mut self, w: &mut Window) -> Viewer {
        // whether need to play next frames when need
        if self.state == PlayState::Playing {
            let next_frame: Option<FrameImage>;
            // prepare next frame from buf or decoder
            if let Some(fb) = self.frame_buf.take() {
                // if buffer is not none, clear it first
                next_frame = Some(fb);
            } else if let Some(f) = self.consumer.try_pop() {
                next_frame = Some(f);
            } else {
                next_frame = None;
            }

            // if self.is_waiting {
            //     self.is_waiting = false;
            //     self.played_time = Some(frame_time);
            //     self.audio_player.play().unwrap();
            // }
            if self.is_seeking {
                self.play_signal.store(false, Ordering::Release);
                self.audio_player.play().unwrap();
                while !self.play_signal.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(1));
                }
                self.is_seeking = false;
            }

            // update if had next_frame
            if let Some(next_frame) = next_frame {
                match self.compare_time(next_frame.pts) {
                    FrameAction::Wait => {
                        self.frame_buf = Some(next_frame);
                    }
                    FrameAction::Render => {
                        w.drop_image(self.frame.clone()).unwrap();
                        self.frame = next_frame.image;
                    }
                    FrameAction::ReSeek(t) => {
                        if let Some(decoder) = self.decoder.as_mut() {
                            decoder.set_event(DecoderEvent::Seek(t));
                        };
                        w.drop_image(next_frame.image).unwrap();
                        self.consumer.clear();
                        self.frame_buf = None;
                    }
                    FrameAction::Drop => {
                        w.drop_image(next_frame.image).unwrap();
                    }
                }
            }
        }
        Viewer::new(self.frame.clone(), self.size.clone())
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop_play();
    }
}
