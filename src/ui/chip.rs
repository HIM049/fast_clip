use gpui::{Hsla, IntoElement, ParentElement, RenderOnce, Styled, div, prelude::FluentBuilder};
use gpui_component::{ActiveTheme, Colorize, StyledExt};

#[derive(IntoElement)]
pub struct Chip {
    label: Option<String>,
    color: Option<Hsla>,
    border: bool,
    bold: bool,
}

impl Chip {
    pub fn new() -> Self {
        Self {
            label: None,
            color: None,
            border: false,
            bold: false,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn border(mut self) -> Self {
        self.border = true;
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
        let bg_color = self.color.unwrap_or(cx.theme().background.darken(0.5));
        let font_color = cx.theme().foreground;
        div()
            .flex()
            .justify_center()
            .items_center()
            .bg(bg_color)
            .px_5()
            .py_1()
            .rounded_full()
            .when(self.bold, |this| this.font_bold())
            .when(self.border, |this| {
                this.border_1().border_color(gpui::white().alpha(0.3))
            })
            .when_some(self.label, |div, label| {
                div.child(label).text_color(font_color)
            })
    }
}
