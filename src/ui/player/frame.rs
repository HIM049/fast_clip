use std::sync::Arc;

use gpui::RenderImage;

pub struct FrameImage {
    pub image: Arc<RenderImage>,
    pub pts: i64,
    pub reseeked: bool,
}

#[derive(Debug)]
pub enum FrameAction {
    Wait,
    Render,
    // ReSeek(f64),
    Drop,
}
