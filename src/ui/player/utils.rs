use std::sync::Arc;

use gpui::{App, IntoElement, ParentElement, RenderImage, Styled, Window, div};
use gpui_component::Root;
use image::RgbaImage;

pub fn generate_image_fallback(size: (u32, u32), frame: Vec<u8>) -> Arc<RenderImage> {
    let frame_len = frame.len();

    if let Some(buff) = RgbaImage::from_vec(size.0, size.1, frame) {
        let frame_img = image::Frame::new(buff);
        Arc::new(RenderImage::new(vec![frame_img]))
    } else {
        println!(
            "DEBUG: fallbacked: frame len {}, size {:?}",
            frame_len, size
        );
        let frame = vec![0, 0, 0, 0].repeat((size.0 * size.1) as usize);
        generate_image_fallback(size, frame)
    }
}

pub fn render_notification_layer(
    window: &mut Window,
    cx: &mut App,
) -> Option<impl IntoElement + use<>> {
    let root = window.root::<Root>()??;

    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .flex()
            .justify_center()
            .child(root.read(cx).notification.clone()),
    )
}
