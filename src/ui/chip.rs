use gpui::{IntoElement, ParentElement, RenderOnce, Styled, div, prelude::FluentBuilder};
use gpui_component::StyledExt;

#[derive(IntoElement)]
pub struct Chip {
    label: Option<String>,
}

impl Chip {
    pub fn new() -> Self {
        Self { label: None }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _: &mut gpui::Window, _: &mut gpui::App) -> impl gpui::IntoElement {
        div()
            .flex()
            .justify_center()
            .items_center()
            .border_1()
            .border_color(gpui::black())
            .bg(gpui::black())
            .px_5()
            .py_1()
            .rounded_full()
            .font_bold()
            .when_some(self.label, |div, label| div.child(label))
    }
}
