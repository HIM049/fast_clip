use std::{ops::Range, sync::Arc};

use gpui::{
    AbsoluteLength, App, BorderStyle, Bounds, Corners, DefiniteLength, Element, ElementId, Hsla,
    IntoElement, LayoutId, Length, MouseButton, MouseDownEvent, Path, Pixels, Point, Size, Style,
    Window, point, px, quad, relative, rgb, size,
};

pub struct Timeline {
    id: ElementId,
    percent: f32,
    origin_point: Point<Pixels>,
    on_click: Option<Arc<Box<dyn Fn(f32, &mut App) + 'static>>>,
    range: Range<Option<f32>>,
}

impl Timeline {
    pub fn new(id: impl Into<ElementId>, percent: f32, range: Range<Option<f32>>) -> Self {
        Self {
            id: id.into(),
            percent: percent,
            origin_point: point(px(0.), px(0.)),
            on_click: None,
            range,
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
        let scale = window.scale_factor();

        let base_played = rgb(0x0091FF);
        let base_unplay = rgb(0x005CA3);
        let base_h = px(10.);

        let selected_played = rgb(0xf0e59a);
        let selected_unplay = rgb(0x978F5C);

        let point_color = rgb(0xFFF29A);

        let indi_x = self.indicator_x(bounds);

        // timeline base
        window.paint_quad(quad(
            Bounds {
                origin: self.origin_point,
                size: Size {
                    width: bounds.size.width,
                    height: base_h,
                },
            },
            Corners::default(),
            base_unplay,
            px(0.),
            gpui::white(),
            BorderStyle::default(),
        ));
        // timebase played
        window.paint_quad(quad(
            Bounds {
                origin: self.origin_point,
                size: Size {
                    width: indi_x - px(1.),
                    height: base_h,
                },
            },
            Corners::default(),
            base_played,
            px(0.),
            gpui::white(),
            BorderStyle::default(),
        ));

        // selected range
        if self.range.start.is_some() || self.range.end.is_some() {
            let start = self.range.start.unwrap_or(0.);
            let end = self.range.end.unwrap_or(1.);
            let point_a = (bounds.size.width * start).max(bounds.origin.x);
            let point_b = (bounds.size.width + px(1.)) * end;

            let divide_point = if indi_x > point_b {
                // indicator in range
                point_b
            } else if indi_x > point_a {
                // indicator out range
                indi_x
            } else {
                // indicator before range
                point_a
            };

            if point_a < point_b {
                // unplay range
                window.paint_quad(quad(
                    Bounds {
                        origin: point(point_a.round(), self.origin_point.y),
                        size: Size {
                            width: point_b - point_a,
                            height: base_h,
                        },
                    },
                    Corners::default(),
                    selected_unplay,
                    px(0.),
                    gpui::white(),
                    BorderStyle::default(),
                ));
                // played range
                window.paint_quad(quad(
                    Bounds {
                        origin: point(point_a, self.origin_point.y),
                        size: Size {
                            width: divide_point - point_a,
                            height: base_h,
                        },
                    },
                    Corners::default(),
                    selected_played,
                    px(0.),
                    gpui::white(),
                    BorderStyle::default(),
                ));
            }
        }

        // draw range start point
        if let Some(start) = self.range.start {
            let point = (bounds.size.width * start).round() - px(1.);
            paint_dashline(window, point, self.origin_point.y - px(7.5), point_color);
        }
        // draw range end point
        if let Some(end) = self.range.end {
            let point = (bounds.size.width * end).round();
            paint_dashline(window, point, self.origin_point.y - px(7.5), point_color);
        }

        print_indicator(
            window,
            scale,
            &self.origin_point,
            base_h,
            indi_x,
            gpui::white(),
        );

        let mouse_pos = window.mouse_position();
        if bounds.contains(&mouse_pos) {
            print_indicator(
                window,
                scale,
                &self.origin_point,
                base_h,
                mouse_pos.x,
                gpui::white().alpha(0.5),
            );
        }

        if let Some(on_click) = self.on_click.clone() {
            window.on_mouse_event(move |e: &MouseDownEvent, phase, _, cx| {
                if phase.bubble() && e.button == MouseButton::Left && bounds.contains(&e.position) {
                    let percent = e.position.x / bounds.size.width;
                    on_click(percent, cx);
                    cx.stop_propagation();
                }
            });
        }
    }
}

impl IntoElement for Timeline {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn print_indicator(
    window: &mut Window,
    scale: f32,
    origin_point: &Point<Pixels>,
    base_h: Pixels,
    x: Pixels,
    color: impl Into<Hsla>,
) {
    let color = color.into();
    let head_size = px(5.0 / scale); // triangle size
    let width = px(1.0 / scale);
    let height = px(20.);
    let indi_y = origin_point.y - px(14.);

    let mut path = Path::new(*origin_point);

    // paint triangle
    path.move_to(point(x - head_size, indi_y)); // left top
    path.line_to(point(x + head_size, indi_y)); // right top
    path.line_to(point(x, indi_y + head_size)); // bottom corner
    path.line_to(point(x - head_size, indi_y)); // back to start
    window.paint_path(path, color);

    // paint indicator line
    window.paint_quad(quad(
        Bounds {
            origin: point(x - width / 2.0, origin_point.y - (height - base_h) / 2.),
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
}

fn paint_dashline(window: &mut Window, x: Pixels, y_start: Pixels, color: impl Into<Hsla>) {
    let dash_height = px(4.0); // 每段虚线的高度
    let gap_height = px(1.0); // 间隔高度
    let width = px(1.5); // 线宽
    let height = px(25.);

    let mut current_y = y_start;
    let end_y = y_start + height;

    let color = color.into();

    while current_y < end_y {
        let current_dash_h = if current_y + dash_height > end_y {
            end_y - current_y
        } else {
            dash_height
        };

        window.paint_quad(quad(
            Bounds {
                origin: point(x, current_y),
                size: size(width, current_dash_h),
            },
            Corners::default(),
            color,
            px(0.),
            gpui::white(),
            BorderStyle::default(),
        ));

        current_y += dash_height + gap_height;
    }
}
