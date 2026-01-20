use gpui::{IntoElement, ParentElement, RenderOnce, Styled, div, prelude::FluentBuilder};
use gpui_component::{ActiveTheme, Colorize, StyledExt};

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
    fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        let bg_color = cx.theme().background.darken(0.5);
        let font_color = cx.theme().foreground;
        div()
            .flex()
            .justify_center()
            .items_center()
            .bg(bg_color)
            .px_5()
            .py_1()
            .rounded_full()
            .font_bold()
            .when_some(self.label, |div, label| {
                div.child(label).text_color(font_color)
            })
    }
}
