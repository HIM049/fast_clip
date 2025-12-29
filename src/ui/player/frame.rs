use std::sync::Arc;

use gpui::RenderImage;

pub struct FrameImage {
    pub image: Arc<RenderImage>,
    pub pts: u64,
}

pub enum FrameAction {
    Wait,
    Render,
    ReSeek(f32),
}
