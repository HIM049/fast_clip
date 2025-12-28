use std::{
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use gpui::{Context, Entity, RenderImage, Window};
use ringbuf::{
    HeapCons,
    storage::Heap,
    traits::{Consumer, Split},
};

use crate::ui::{
    app::MyApp,
    player::{
        ffmpeg::VideoDecoder, frame::FrameImage, player_size::PlayerSize,
        utils::generate_image_fallback, viewer::Viewer,
    },
};

#[derive(PartialEq, Clone, Copy)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

pub struct Player {
    size: Entity<PlayerSize>,
    decoder: VideoDecoder,
    frame: Arc<RenderImage>,
    frame_buf: Option<FrameImage>,
    consumer: HeapCons<FrameImage>,
    start_time: Option<Instant>,
    played_time: Option<f32>,
    state: PlayState,
}

impl Player {
    pub fn new(size_entity: Entity<PlayerSize>) -> Self {
        let rb = ringbuf::SharedRb::<Heap<FrameImage>>::new(30 * 1);
        let (producer, consumer) = rb.split();
        Self {
            size: size_entity.clone(),
            decoder: VideoDecoder::new(size_entity).set_producer(producer),
            frame: generate_image_fallback((1, 1), vec![]),
            frame_buf: None,
            consumer,
            start_time: None,
            played_time: None,
            state: PlayState::Stopped,
        }
    }

    pub fn open(&mut self, cx: &mut Context<MyApp>) {
        self.decoder
            .open(
                cx,
                "D:/Videos/Records/Apex Legends 2024.05.04 - 18.07.10.04.DVR.mp4".into(),
            )
            .unwrap();
    }

    pub fn start_play(&mut self, cx: &mut Context<MyApp>) {
        self.state = PlayState::Playing;
        self.decoder.spawn_decoder(self.size.clone(), cx);
    }

    pub fn resume_play(&mut self) {
        self.state = PlayState::Playing;
    }

    pub fn pause_play(&mut self) {
        self.state = PlayState::Paused;
        self.pause_timer();
    }

    pub fn stop_play(&mut self) {
        self.state = PlayState::Stopped;
        self.start_time = None;
        self.frame = generate_image_fallback((1, 1), vec![]);
        self.frame_buf = None;
        self.decoder.need_stop.store(true, Ordering::Relaxed);
    }

    pub fn get_state(&self) -> PlayState {
        self.state
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

    fn compare_time(&mut self, frame_pts: u64) -> bool {
        if self.start_time.is_none() {
            self.start_time = Some(std::time::Instant::now());
        }
        let Some(time) = self.start_time else {
            return false;
        };
        let Some(time_base) = self.decoder.get_timebase() else {
            return false;
        };

        let play_time = time.elapsed().as_secs_f32() + self.played_time.unwrap_or(0.);
        let frame_time = frame_pts as f32 / time_base.denominator() as f32;

        if frame_time <= play_time {
            println!(
                "DEBUG: TIME SYNC - frame_time: {:6.2} | play_time: {:6.2}",
                frame_time, play_time
            );
        }

        frame_time <= play_time
    }

    pub fn view(&mut self, w: &mut Window) -> Viewer {
        if self.state == PlayState::Playing {
            if let Some(fb) = self.frame_buf.take() {
                if self.compare_time(fb.pts) {
                    w.drop_image(self.frame.clone()).unwrap();
                    self.frame = fb.image;
                } else {
                    self.frame_buf = Some(fb);
                }
            } else {
                if let Some(f) = self.consumer.try_pop() {
                    if self.compare_time(f.pts) {
                        w.drop_image(self.frame.clone()).unwrap();
                        self.frame = f.image;
                    } else {
                        self.frame_buf = Some(f);
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
