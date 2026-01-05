use std::{path::PathBuf, sync::Arc, time::Instant};

use gpui::{Context, Entity, RenderImage, Window};
use ringbuf::{
    HeapCons,
    storage::Heap,
    traits::{Consumer, Split},
};

use crate::{
    models::model::OutputParams,
    ui::{
        player::{
            ffmpeg::{DecoderEvent, VideoDecoder},
            frame::{FrameAction, FrameImage},
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
    size: Entity<PlayerSize>,
    // output_params: Entity<OutputParams>,
    decoder: VideoDecoder,
    frame: Arc<RenderImage>,
    frame_buf: Option<FrameImage>,
    consumer: HeapCons<FrameImage>,
    start_time: Option<Instant>,
    played_time: Option<f32>,
    state: PlayState,
}

impl Player {
    pub fn new(size_entity: Entity<PlayerSize>, output_params: Entity<OutputParams>) -> Self {
        let rb = ringbuf::SharedRb::<Heap<FrameImage>>::new(30 * 1);
        let (producer, consumer) = rb.split();
        Self {
            size: size_entity.clone(),
            // output_params: output_params.clone(),
            decoder: VideoDecoder::new(size_entity, output_params).set_producer(producer),
            frame: generate_image_fallback((1, 1), vec![]),
            frame_buf: None,
            consumer,
            start_time: None,
            played_time: None,
            state: PlayState::Stopped,
        }
    }

    pub fn open(&mut self, cx: &mut Context<MyApp>, path: &PathBuf) -> anyhow::Result<()> {
        self.decoder.open(cx, path)?;
        Ok(())
    }

    pub fn start_play(&mut self, cx: &mut Context<MyApp>) {
        self.state = PlayState::Playing;
        self.decoder.spawn_decoder(self.size.clone(), cx);
    }

    pub fn resume_play(&mut self) {
        self.state = PlayState::Playing;
        self.decoder.set_event(DecoderEvent::None);
    }

    pub fn pause_play(&mut self) {
        self.state = PlayState::Paused;
        self.decoder.set_event(DecoderEvent::Pause);
        self.pause_timer();
    }

    pub fn stop_play(&mut self) {
        self.state = PlayState::Stopped;
        self.start_time = None;
        self.frame = generate_image_fallback((1, 1), vec![]);
        self.frame_buf = None;
        self.decoder.set_event(DecoderEvent::Stop);
    }

    pub fn set_playtime<F>(&mut self, update_fn: F)
    where
        F: Fn(f32, f32) -> f32,
    {
        self.pause_timer();
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
        let Some(duration) = self.decoder.get_duration() else {
            return None;
        };
        let Some(timebase) = self.decoder.get_timebase() else {
            return None;
        };
        if duration.is_negative() {
            return None;
        }
        let d_sec = (duration as f64 / timebase.denominator() as f64) as f32;
        Some(self.current_playtime() / d_sec)
    }

    pub fn duration_sec(&self) -> Option<f32> {
        let Some(duration) = self.decoder.get_duration() else {
            return None;
        };
        let Some(timebase) = self.decoder.get_timebase() else {
            return None;
        };
        Some((duration as f64 / timebase.denominator() as f64) as f32)
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

    fn compare_time(&mut self, frame_pts: u64) -> FrameAction {
        if self.start_time.is_none() {
            self.start_time = Some(std::time::Instant::now());
        }
        let Some(time) = self.start_time else {
            return FrameAction::Wait;
        };
        let Some(time_base) = self.decoder.get_timebase() else {
            return FrameAction::Wait;
        };

        let play_time = time.elapsed().as_secs_f32() + self.played_time.unwrap_or(0.);
        let frame_time = frame_pts as f32 / time_base.denominator() as f32;

        if frame_time <= play_time {}

        if (play_time - frame_time).abs() <= 0.3 {
            if frame_time <= play_time {
                FrameAction::Render
            } else {
                FrameAction::Wait
            }
        } else {
            FrameAction::ReSeek(play_time)
        }
    }

    pub fn view(&mut self, w: &mut Window) -> Viewer {
        // whether need to play next frames when need
        if self.state == PlayState::Playing {
            // if buffer is not none, clear it first
            if let Some(fb) = self.frame_buf.take() {
                match self.compare_time(fb.pts) {
                    FrameAction::Wait => {
                        self.frame_buf = Some(fb);
                    }
                    FrameAction::Render => {
                        w.drop_image(self.frame.clone()).unwrap();
                        self.frame = fb.image;
                    }
                    FrameAction::ReSeek(t) => {
                        self.decoder.set_event(DecoderEvent::Seek(t));
                        w.drop_image(fb.image).unwrap();
                        self.consumer.clear();
                    }
                }
            } else {
                if let Some(f) = self.consumer.try_pop() {
                    match self.compare_time(f.pts) {
                        FrameAction::Wait => {
                            self.frame_buf = Some(f);
                        }
                        FrameAction::Render => {
                            w.drop_image(self.frame.clone()).unwrap();
                            self.frame = f.image;
                        }
                        FrameAction::ReSeek(t) => {
                            self.decoder.set_event(DecoderEvent::Seek(t));
                            w.drop_image(f.image).unwrap();
                            self.consumer.clear();
                        }
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
