use std::path::PathBuf;

use gpui::{Action, App, SharedString, WindowHandle};
use gpui_component::Root;

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

pub struct OutputParams {
    pub path: Option<PathBuf>,
    pub video_stream_ix: Option<usize>,
    pub audio_stream_ix: Option<usize>,
    pub selected_range: Option<(f32, f32)>,
}

impl OutputParams {
    pub fn default() -> Self {
        Self {
            path: None,
            video_stream_ix: None,
            audio_stream_ix: None,
            selected_range: None,
        }
    }
}

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = story, no_json)]
pub struct SelectLocale(pub SharedString);
