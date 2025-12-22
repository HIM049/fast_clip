use std::{sync::Arc, vec};

use gpui::{
    Bounds, Context, Corners, Element, Entity, IntoElement, LayoutId, Pixels, Point, RenderImage,
    Size, Style, px, relative,
};
use gpui_component::PixelsExt;
use image::RgbaImage;

use crate::ui::{app::MyApp, player_size::PlayerSize};

pub struct Viewer {
    size: Entity<PlayerSize>,
    frame: Arc<RenderImage>,
}

impl Viewer {
    pub fn new(
        cx: &mut Context<MyApp>,
        frame: Arc<Vec<u8>>,
        size_entity: Entity<PlayerSize>,
    ) -> Self {
        let size = size_entity.read(cx);

        Self {
            size: size_entity,
            frame: Self::generate_image_fallback(
                (size.orignal_size().0, size.orignal_size().1),
                frame.to_vec(),
            ),
        }
    }

    fn generate_image_fallback(size: (u32, u32), frame: Vec<u8>) -> Arc<RenderImage> {
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
            Self::generate_image_fallback(size, frame)
        }
    }
}

impl Element for Viewer {
    type RequestLayoutState = LayoutId;

    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();

        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();

        let layout_id = window.request_layout(style, None, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        let w = bounds.size.width.to_f64().round() as u32;
        let h = bounds.size.height.to_f64().round() as u32;

        self.size.update(cx, |size, _| {
            size.set_view((w, h));
        })
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        let size = self.size.read(cx);

        let x = (size.view_size().0 - size.output_size().0) / 2;
        let y = (size.view_size().1 - size.output_size().1) / 2;
        let xp = px(bounds.origin.x.as_f32() + x as f32);
        let yp = px(bounds.origin.y.as_f32() + y as f32);

        let b = Bounds::<Pixels>::new(
            Point::new(xp, yp),
            Size::<Pixels>::new(
                px(size.output_size().0 as f32),
                px(size.output_size().1 as f32),
            ),
        );
        window
            .paint_image(b, Corners::all(px(0.0)), self.frame.clone(), 0, false)
            .unwrap();
    }
}

impl IntoElement for Viewer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
