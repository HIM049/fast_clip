use std::sync::Arc;

use app_assets::icons::rounded;
use gpui::{
    Bounds, Corners, Element, Entity, IntoElement, LayoutId, Pixels, Point, RenderImage, Size,
    Style, TransformationMatrix, px, relative,
};

use crate::ui::player::size::PlayerSize;

pub struct Viewer {
    size: Entity<PlayerSize>,
    frame: Arc<RenderImage>,
    slow_mode: bool,
}

impl Viewer {
    pub fn new(frame: Arc<RenderImage>, size_entity: Entity<PlayerSize>, slow_mode: bool) -> Self {
        Self {
            size: size_entity,
            frame,
            slow_mode,
        }
    }
}

impl Element for Viewer {
    type RequestLayoutState = LayoutId;

    type PrepaintState = Bounds<Pixels>;

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
        });

        let size = self.size.read(cx);

        let x = (size.view_size().0 - size.output_size().0) / 2;
        let y = (size.view_size().1 - size.output_size().1) / 2;
        let xp = px(bounds.origin.x.as_f32() + x as f32);
        let yp = px(bounds.origin.y.as_f32() + y as f32);

        Bounds::<Pixels>::new(
            Point::new(xp, yp),
            Size::<Pixels>::new(
                px(size.output_size().0 as f32),
                px(size.output_size().1 as f32),
            ),
        )
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _: &mut Self::RequestLayoutState,
        b: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        window
            .paint_image(*b, *b, Corners::all(px(0.0)), self.frame.clone(), 0, false)
            .unwrap();

        if self.slow_mode {
            let icon_size = px(24.0);
            let margin = px(12.0);
            let icon_bounds = Bounds::<Pixels>::new(
                Point::new(bounds.right() - icon_size - margin, bounds.top() + margin),
                Size::new(icon_size, icon_size),
            );
            window
                .paint_svg(
                    icon_bounds,
                    rounded::SPEED_2.into(),
                    None,
                    TransformationMatrix::unit(),
                    gpui::red(),
                    cx,
                )
                .unwrap();
        }
    }
}

impl IntoElement for Viewer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
