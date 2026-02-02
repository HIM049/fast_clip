use std::{ops::Range, path::PathBuf};

use gpui::{App, WindowHandle};
use gpui_component::Root;

use crate::ui::player::model::AudioRail;

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
    pub selected_range: Option<Range<f64>>,
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
