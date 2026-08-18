use gpui::{
    AnyElement, Context, Decorations, InteractiveElement, IntoElement, Keystroke, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::{ActiveTheme as _, StyledExt, TitleBar, kbd::Kbd, label::Label};
use rust_i18n::t;

use crate::config::keymap;

pub struct HelpView;

impl HelpView {
    pub fn new() -> Self {
        Self
    }
}

impl Render for HelpView {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_bar = (cfg!(target_os = "linux")
            && matches!(w.window_decorations(), Decorations::Client { .. }))
        .then(|| TitleBar::new().text_sm().child(t!("help.title")));

        let bg_color = cx.theme().background;
        div()
            .size_full()
            .bg(bg_color)
            .v_flex()
            .children(title_bar)
            .child(
                div()
                    .id("help-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .v_flex()
                            .p_6()
                            .gap_8()
                            .child(
                                div().w_full().flex().justify_start(),
                                // .child(Label::new(t!("help.title")).font_bold().text_2xl()),
                            )
                            .children(sections()),
                    ),
            )
    }
}

fn sections() -> Vec<AnyElement> {
    vec![
        section(
            t!("help.playback.title"),
            vec![
                shortcut_row("help.playback.play_desc", keymap::PLAY_PAUSE),
                shortcut_row("help.playback.back_desc", keymap::SEEK_BACKWARD),
                shortcut_row("help.playback.forward_desc", keymap::SEEK_FORWARD),
                shortcut_row("help.playback.volume_up_desc", keymap::VOLUME_UP),
                shortcut_row("help.playback.volume_down_desc", keymap::VOLUME_DOWN),
            ],
        ),
        section(
            t!("help.range.title"),
            vec![
                shortcut_row("help.range.set_start_desc", keymap::SET_RANGE_START),
                shortcut_row("help.range.set_end_desc", keymap::SET_RANGE_END),
            ],
        ),
        section(
            t!("help.export.title"),
            vec![shortcut_row("help.export.export_desc", keymap::EXPORT)],
        ),
    ]
}

fn section(title: impl Into<SharedString>, rows: Vec<AnyElement>) -> AnyElement {
    div()
        .w_full()
        .v_flex()
        .gap_3()
        .child(
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(Label::new(title).font_bold().text_xl()),
        )
        .child(div().w_full().grid().grid_cols(2).gap_3().children(rows))
        .into_any_element()
}

fn shortcut_row(desc_key: &str, key: &str) -> AnyElement {
    div()
        .w_full()
        .h_flex()
        .items_center()
        .justify_center()
        .gap_3()
        .child(Kbd::new(Keystroke::parse(key).expect("invalid help keystroke")).text_xl())
        .child(Label::new(i18n(desc_key)).text_base())
        .into_any_element()
}

fn i18n(key: &str) -> SharedString {
    t!(key).to_string().into()
}
