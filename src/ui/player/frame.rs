use std::sync::Arc;

use gpui::RenderImage;

use crate::ui::player::utils::generate_image_fallback;

pub struct FrameImage {
    pub image: Arc<RenderImage>,
    pub pts: i64,
    pub reseeked: bool,
}

impl FrameImage {
    pub fn default() -> Self {
        Self {
            image: generate_image_fallback((1, 1), vec![]),
            pts: 0,
            reseeked: false,
        }
    }
}

pub struct FrameAudio {
    pub sample: Arc<Vec<f32>>,
    pub pts: i64,
}

#[derive(Debug)]
pub enum FrameAction {
    Wait,
    Render,
    // ReSeek(f64),
    Drop,
}
