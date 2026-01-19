use gpui::{
    App, AppContext, Bounds, Entity, ParentElement, Render, Styled, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, px, size,
};
use gpui_component::{
    IndexPath, Root, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    label::Label,
    select::{Select, SelectState},
};

use crate::ui::player::ffmpeg::AudioRail;

pub struct PlayerSettings {
    pub audio_ix: usize,
    pub audio_rails: Vec<AudioRail>,
}

impl PlayerSettings {
    pub fn default() -> Self {
        Self {
            audio_ix: 0,
            audio_rails: vec![],
        }
    }
}

pub struct PlayerSettingsView {
    settings: Entity<PlayerSettings>,
    audio_select: Entity<SelectState<Vec<AudioRail>>>,
}

impl PlayerSettingsView {
    fn new(cx: &mut App, window: &mut Window, settings: Entity<PlayerSettings>) -> Self {
        let s = settings.read(cx);
        let rails = s.audio_rails.clone();
        let selected_index = Some(IndexPath::new(s.audio_ix));
        let select = cx.new(|cx| SelectState::new(rails, selected_index, window, cx));
        Self {
            settings: settings,
            audio_select: select,
        }
    }

    pub fn open_window(cx: &mut App, handle: Entity<PlayerSettings>) -> anyhow::Result<()> {
        let window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(400.), px(300.)),
            cx,
        )));
        cx.open_window(
            WindowOptions {
                window_bounds,
                titlebar: Some(TitlebarOptions {
                    title: Some("Player Settings".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                focus: true,
                show: true,
                is_resizable: false,
                is_minimizable: false,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| PlayerSettingsView::new(cx, window, handle));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )?;

        Ok(())
    }
}

impl Render for PlayerSettingsView {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .size_full()
            .flex()
            .v_flex()
            .p_3()
            .justify_between()
            .child(
                div().flex().v_flex().gap_3().child(
                    div()
                        .child(Label::new("Audio Rail"))
                        .child(Select::new(&self.audio_select)),
                ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel")
                            .small()
                            .label("Cancel")
                            .on_click(|_, w, _| {
                                w.remove_window();
                            }),
                    )
                    .child(Button::new("apply").small().primary().label("Ok").on_click(
                        cx.listener(|this, _, w, cx| {
                            if let Some(v) = this.audio_select.read(cx).selected_value() {
                                let v = v.clone();
                                this.settings.update(cx, |s, cx| {
                                    s.audio_ix = v;
                                    cx.notify();
                                })
                            }
                            w.remove_window();
                        }),
                    )),
            )
    }
}
