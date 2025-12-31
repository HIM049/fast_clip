use std::sync::Arc;

use gpui::{
    AbsoluteLength, App, BorderStyle, Bounds, Corners, DefiniteLength, Element, ElementId,
    IntoElement, LayoutId, Length, MouseDownEvent, Path, Pixels, Point, Size, Style, point, px,
    quad, relative, rgb,
};

pub struct Timeline {
    id: ElementId,
    percent: f32,
    origin_point: Point<Pixels>,
    on_click: Option<Arc<Box<dyn Fn(f32, &mut App) + 'static>>>,
}

impl Timeline {
    pub fn new(id: impl Into<ElementId>, percent: f32) -> Self {
        Self {
            id: id.into(),
            percent: percent,
            origin_point: point(px(0.), px(0.)),
            on_click: None,
        }
    }

    pub fn on_click(mut self, handler: impl Fn(f32, &mut App) + 'static) -> Self {
        self.on_click = Some(Arc::new(Box::new(handler)));
        self
    }

    fn indicator_x(&self, b: gpui::Bounds<gpui::Pixels>) -> Pixels {
        (b.size.width * self.percent).round()
    }
}

impl Element for Timeline {
    type RequestLayoutState = LayoutId;

    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        Some(self.id.clone())
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
        style.size.height =
            Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(50.))));

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
        _: &mut gpui::App,
    ) -> Self::PrepaintState {
        self.origin_point = point(bounds.origin.x, bounds.origin.y + px(20.))
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        _: &mut gpui::App,
    ) {
        // timeline base
        window.paint_quad(quad(
            Bounds {
                origin: self.origin_point,
                size: Size {
                    width: bounds.size.width,
                    height: px(10.),
                },
            },
            Corners::default(),
            rgb(0x65acd7),
            px(0.),
            gpui::white(),
            BorderStyle::default(),
        ));

        // triangle size
        let scale = window.scale_factor();
        let head_size = px(5.0 / scale);

        let width = px(1.0 / scale);
        let height = px(16.);
        let x = self.indicator_x(bounds);
        let y = self.origin_point.y - px(14.);
        let color = gpui::white();
        let mut path = Path::new(self.origin_point);

        // paint triangle
        path.move_to(point(x - head_size, y)); // left top
        path.line_to(point(x + head_size, y)); // right top
        path.line_to(point(x, y + head_size)); // bottom corner
        path.line_to(point(x - head_size, y)); // back to start
        window.paint_path(path, color);

        // paint indicator line
        window.paint_quad(quad(
            Bounds {
                origin: point(x - width / 2.0, self.origin_point.y - px(3.)),
                size: Size {
                    width: width,
                    height: height,
                },
            },
            Corners::default(),
            color,
            px(0.),
            color,
            BorderStyle::default(),
        ));

        let on_click = self.on_click.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, _, cx| {
            if phase.bubble() && bounds.contains(&e.position) {
                let percent = e.position.x / bounds.size.width;
                if let Some(handler) = on_click.as_ref() {
                    (handler)(percent, cx);
                }
                cx.stop_propagation();
            }
        });
    }
}

impl IntoElement for Timeline {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
