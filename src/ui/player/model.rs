use std::sync::Arc;

use gpui::{RenderImage, SharedString};
use gpui_component::select::SelectItem;

pub struct FrameImage {
    pub image: Arc<RenderImage>,
    pub pts: i64,
    pub reseeked: bool,
}

#[derive(Debug)]
pub enum FrameAction {
    Wait,
    Render,
    Drop,
}

#[derive(Debug, Clone)]
pub struct AudioRail {
    pub code: usize,
    pub ix: usize,
    pub id: usize,
    pub duration: i64,
    pub handler_name: Option<SharedString>,
}

impl SelectItem for AudioRail {
    type Value = usize;

    fn title(&self) -> gpui::SharedString {
        let name = match self.handler_name.clone() {
            Some(n) => n,
            None => "_".into(),
        };
        SharedString::new(format!("rail-{} ({})", self.code, name))
    }

    fn value(&self) -> &Self::Value {
        &self.ix
    }
}
