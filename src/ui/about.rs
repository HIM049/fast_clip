use gpui::{ParentElement, Render, Styled, div};

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
            .child("About")
    }
}
