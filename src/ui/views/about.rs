use app_assets::icons;
use gpui::{ParentElement, Render, Styled, div, svg};
use gpui_component::{Icon, StyledExt};

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
                    .path(icons::rounded::PAUSE_FILLED)
                    .size_16()
                    .text_color(gpui::white()),
            )
            .child(Icon::new(Icon::empty()).path(icons::rounded::PAUSE_FILLED))
    }
}
