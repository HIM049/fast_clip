use std::{path::PathBuf, sync::Arc, time::Instant};

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
            frame::{FrameAction, FrameAudio, FrameImage},
            size::PlayerSize,
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
    size: Entity<PlayerSize>,
    output_params: Entity<OutputParams>,
    decoder: Option<VideoDecoder>,
    frame: Arc<RenderImage>,
    frame_buf: Option<FrameImage>,
    producer: Option<HeapProd<FrameImage>>,
    a_producer: Option<HeapProd<FrameAudio>>,
    consumer: HeapCons<FrameImage>,
    start_time: Option<Instant>,
    played_time: Option<f32>,
    state: PlayState,

    audio_player: AudioPlayer,

    recent_pts: f32,
    is_time_set: bool,
    is_seeking: bool,
}

impl Player {
    pub fn new(size_entity: Entity<PlayerSize>, output_params: Entity<OutputParams>) -> Self {
        let rb = ringbuf::SharedRb::<Heap<FrameImage>>::new(30 * 1);
        let (v_producer, v_consumer) = rb.split();

        let rb = ringbuf::SharedRb::<Heap<FrameAudio>>::new(30 * 1);
        let (a_producer, a_consumer) = rb.split();

        let audio_player = AudioPlayer::new().unwrap().spawn(a_consumer);

        Self {
            init: false,
            size: size_entity.clone(),
            output_params: output_params.clone(),
            decoder: None,
            frame: generate_image_fallback((1, 1), vec![]),
            frame_buf: None,
            producer: Some(v_producer),
            a_producer: Some(a_producer),
            consumer: v_consumer,
            start_time: None,
            played_time: None,
            state: PlayState::Stopped,

            audio_player,

            recent_pts: 0.0,
            is_time_set: false,
            is_seeking: false,
        }
    }

    pub fn is_init(&self) -> bool {
        self.init
    }

    pub fn open<T>(&mut self, cx: &mut Context<T>, path: &PathBuf) -> anyhow::Result<()>
    where
        T: 'static,
    {
        self.decoder = Some(
            VideoDecoder::open(
                cx,
                path,
                self.size.clone(),
                self.output_params.clone(),
                self.audio_player.sample_rate(),
            )
            .unwrap()
            .set_video_producer(self.producer.take().unwrap())
            .set_audio_producer(self.a_producer.take().unwrap()),
        );
        self.init = true;
        Ok(())
    }

    pub fn start_play(&mut self, cx: &mut Context<MyApp>) {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.spawn_decoder(self.size.clone(), cx);
            self.state = PlayState::Playing;
            self.start_timer();
        }
    }

    pub fn resume_play(&mut self) {
        self.state = PlayState::Playing;
        self.start_timer();
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.set_event(DecoderEvent::None);
        }
    }

    pub fn pause_play(&mut self) {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.set_event(DecoderEvent::Pause);
            self.state = PlayState::Paused;
            self.pause_timer();
        }
    }

    pub fn stop_play(&mut self) {
        self.state = PlayState::Stopped;
        self.start_time = None;
        self.frame = generate_image_fallback((1, 1), vec![]);
        self.frame_buf = None;
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.set_event(DecoderEvent::Stop);
        }
        // TODO: drop decoder
    }

    pub fn set_playtime<F>(&mut self, update_fn: F)
    where
        F: Fn(f32, f32) -> f32,
    {
        self.pause_timer();
        self.is_time_set = true;
        let now = self.played_time.unwrap_or(0.);
        let dur_sec = self.duration_sec().unwrap_or(0.);
        self.played_time = Some(update_fn(now, dur_sec).clamp(0.0, dur_sec));
    }

    pub fn get_state(&self) -> PlayState {
        self.state
    }

    pub fn current_playtime(&self) -> f32 {
        if let Some(start_time) = self.start_time {
            start_time.elapsed().as_secs_f32() + self.played_time.unwrap_or(0.0)
        } else {
            self.played_time.unwrap_or(0.0)
        }
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
        Some(self.current_playtime() / d_sec)
    }

    pub fn duration_sec(&self) -> Option<f32> {
        let Some(decoder) = self.decoder.as_ref() else {
            return None;
        };
        let Some(duration) = decoder.get_duration() else {
            return None;
        };
        let timebase = decoder.get_timebase();
        Some((duration as f64 / timebase.denominator() as f64) as f32)
    }

    fn start_timer(&mut self) {
        self.start_time = Some(std::time::Instant::now());
    }

    fn pause_timer(&mut self) {
        let Some(start_time) = self.start_time.take() else {
            return;
        };
        if let Some(played) = self.played_time.take() {
            self.played_time = Some(played + start_time.elapsed().as_secs_f32());
        } else {
            self.played_time = Some(start_time.elapsed().as_secs_f32());
        }
    }

    fn play_time(&self) -> f32 {
        let time_sec;
        if let Some(time) = self.start_time {
            time_sec = time.elapsed().as_secs_f32();
        } else {
            time_sec = 0.;
        }
        self.played_time.unwrap_or(0.) + time_sec
    }

    fn frame_time(&self, pts: i64) -> Option<f32> {
        if pts.is_negative() {
            return None;
        }
        let Some(decoder) = self.decoder.as_ref() else {
            return None;
        };
        let time_base = decoder.get_timebase();
        Some(pts as f32 / time_base.denominator() as f32)
    }

    fn compare_time(&mut self, frame_pts: i64) -> FrameAction {
        let Some(frame_time) = self.frame_time(frame_pts) else {
            return FrameAction::Wait;
        };
        self.recent_pts = frame_time;
        let play_time = self.play_time();

        if (play_time - frame_time).abs() <= 0.3 {
            if self.is_seeking {
                self.is_seeking = false;
                self.start_timer();
            }
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
                self.pause_timer();
                FrameAction::ReSeek(play_time)
            }
        }
    }

    pub fn dbg_msg(&self) -> String {
        format!(
            "PlayInfo: PT {:.2}, RFT {:.2}, SEEKING {}, UNHANDLE_SET {}, DIFF {:.2}",
            self.play_time(),
            self.recent_pts,
            self.is_seeking,
            self.is_time_set,
            self.play_time() - self.recent_pts
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
