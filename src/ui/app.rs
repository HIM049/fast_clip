use std::path::PathBuf;

use gpui::{
    AppContext, Canvas, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, canvas, div, img, rgb,
};
use gpui_component::{ActiveTheme, StyledExt, Theme, ThemeRegistry};

use crate::components::app_title_bar::AppTitleBar;

pub struct MyApp {
    title_bar: Entity<AppTitleBar>,
}

impl MyApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new("EzClip", cx));

        Self { title_bar }
    }
}

impl Render for MyApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .bg(cx.theme().background)
            .v_flex()
            .size_full()
            .child(self.title_bar.clone())
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .min_h_0()
                    .child(
                        // preview zone
                        div()
                            .flex()
                            .justify_center()
                            .items_center()
                            .size_full()
                            .debug_blue()
                            .child("preview area"),
                    )
                    .child(
                        // control zone
                        div()
                            .flex()
                            .flex_shrink_0()
                            .justify_center()
                            .items_center()
                            .w_full()
                            .h_1_3()
                            .child("control zone"),
                    ),
            )
    }
}
