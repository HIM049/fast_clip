use gpui::{ParentElement, Render, Styled, div, svg};
use gpui_component::StyledExt;

pub struct AboutView;

impl Render for AboutView {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .font_medium()
            .child(
                svg()
                    .path("icons/play_arrow.svg")
                    .size_10()
                    .text_color(gpui::white()),
            )
    }
}
