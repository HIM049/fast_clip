use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Stateful, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, rgba,
};
use gpui_component::{Colorize, StyledExt};

#[derive(IntoElement)]
pub struct RoundButton {
    id: ElementId,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    color: Option<Hsla>,
    label: Option<String>,
    child: Option<AnyElement>,
}

impl RoundButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            on_click: None,
            color: None,
            label: None,
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

    pub fn child(mut self, child: impl Into<AnyElement>) -> Self {
        self.child = Some(child.into());
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
        let default_color: Hsla = rgba(0xffffff40).into();
        div()
            .id(self.id)
            .flex()
            .justify_center()
            .items_center()
            .when_some(self.color, |div, color| div.bg(color))
            .when_none(&self.color, |div| div.bg(default_color))
            .border_1()
            .border_color(rgba(0xffffff4d))
            .px_5()
            .py_1()
            .rounded_full()
            .font_bold()
            .when_some(self.color, |div, color| {
                div.hover(|style| style.bg(color.darken(0.2)))
            })
            .when_none(&self.color, |div| {
                div.hover(|style| style.bg(default_color.darken(0.2)))
            })
            .when_some(self.label, |div, label| div.child(label))
            .when_some(self.child, |div, child| div.child(child))
            .when_some(self.on_click, |div: Stateful<Div>, on_click| {
                div.on_click(
                    move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
                        on_click(event, window, cx);
                    },
                )
            })
    }
}
