use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Div, Element, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Stateful, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, rgba, svg,
};
use gpui_component::{Colorize, StyledExt};

#[derive(IntoElement)]
pub struct RoundButton {
    id: ElementId,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    color: Option<Hsla>,
    label: Option<String>,
    icon: Option<String>,
    small_icon: bool,
    child: Option<AnyElement>,
}

impl RoundButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            on_click: None,
            color: None,
            label: None,
            icon: None,
            small_icon: false,
            child: None,
        }
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn blue(mut self) -> Self {
        self.color = Some(rgba(0x0091ffcc).into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }

    pub fn icon_path(mut self, path: impl Into<String>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn small_icon(mut self) -> Self {
        self.small_icon = true;
        self
    }

    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for RoundButton {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        let bg_color = self.color.unwrap_or(rgba(0xffffff40).into());
        div()
            .id(self.id)
            .flex()
            .justify_center()
            .items_center()
            .bg(bg_color)
            .border_1()
            .border_color(rgba(0xffffff4d))
            .px_3()
            .py_1()
            .rounded_full()
            .font_bold()
            .hover(|style| style.bg(bg_color.darken(0.2)))
            .active(|style| style.bg(bg_color.lighten(0.2)))
            .child(
                div().child(
                    div()
                        .min_h_6()
                        .min_w_6()
                        .flex()
                        .justify_center()
                        .items_center()
                        .when_some(self.icon, |this, path| {
                            this.child(
                                svg()
                                    .path(path)
                                    .size_6()
                                    .when(self.small_icon, |this| this.size_5())
                                    .text_color(gpui::white().alpha(0.8)),
                            )
                        })
                        .when_some(self.child, |this, child| this.child(child))
                        .when_some(self.label, |this, label| this.child(label)),
                ),
            )
            .when_some(self.on_click, |div: Stateful<Div>, on_click| {
                div.on_click(
                    move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
                        on_click(event, window, cx);
                    },
                )
            })
    }
}
