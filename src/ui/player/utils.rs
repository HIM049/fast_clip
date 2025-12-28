use std::sync::Arc;

use gpui::RenderImage;
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
