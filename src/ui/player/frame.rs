use std::sync::Arc;

use gpui::RenderImage;

use crate::ui::player::utils::generate_image_fallback;

pub struct FrameImage {
    pub image: Arc<RenderImage>,
    pub pts: u64,
}

impl FrameImage {
    pub fn default() -> Self {
        Self {
            image: generate_image_fallback((1, 1), vec![]),
            pts: 0,
        }
    }
}

#[derive(Debug)]
pub enum FrameAction {
    Wait,
    Render,
    ReSeek(f32),
    Drop,
}
