use std::path::PathBuf;

use gpui::{Action, App, SharedString, WindowHandle};
use gpui_component::Root;

use crate::ui::player::ffmpeg::AudioRail;

pub struct WindowState {
    pub output_handle: Option<WindowHandle<Root>>,
    pub about_handle: Option<WindowHandle<Root>>,
}

impl WindowState {
    pub fn default() -> Self {
        Self {
            output_handle: None,
            about_handle: None,
        }
    }

    pub fn close_all(&mut self, cx: &mut App) {
        if let Some(h) = self.output_handle {
            h.update(cx, |_, w, cx| {
                w.remove_window();
            })
            .unwrap();
        }
        if let Some(h) = self.about_handle {
            h.update(cx, |_, w, _| {
                w.remove_window();
            })
            .unwrap();
        }
    }
}

#[derive(Debug)]
pub struct OutputParams {
    pub path: Option<PathBuf>,
    pub video_stream_ix: Option<usize>,
    pub audio_stream_ix: Option<usize>,
    pub selected_range: Option<(f64, f64)>,
    pub audio_rails: Option<Vec<AudioRail>>,
}

impl OutputParams {
    pub fn default() -> Self {
        Self {
            path: None,
            video_stream_ix: None,
            audio_stream_ix: None,
            selected_range: None,
            audio_rails: None,
        }
    }

    pub fn all_some(&self) -> bool {
        self.path.is_some()
            && self.video_stream_ix.is_some()
            && self.audio_stream_ix.is_some()
            && self.selected_range.is_some()
            && self.audio_rails.is_some()
    }
}

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = menu, no_json)]
pub struct SelectLocale(pub SharedString);
