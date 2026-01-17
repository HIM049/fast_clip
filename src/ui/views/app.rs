use app_assets::icons;
use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, div, prelude::FluentBuilder,
};
use gpui_component::{ActiveTheme, StyledExt};

use crate::{
    Back, Close, Forward, SetEnd, SetStart, SwitchPlay,
    components::app_title_bar::AppTitleBar,
    models::model::OutputParams,
    ui::{
        button::RoundButton,
        chip::Chip,
        player::{
            player::{PlayState, Player},
            size::PlayerSize,
        },
        timeline::Timeline,
    },
};

pub struct MyApp {
    title_bar: Entity<AppTitleBar>,
    size: Entity<PlayerSize>,
    output_parames: Entity<OutputParams>,
    player: Player,
    // here selection_range is percentage of progress
    selection_range: (Option<f32>, Option<f32>),
    focus_handle: FocusHandle,
}

impl MyApp {
    pub fn new(
        cx: &mut Context<Self>,
        size_entity: Entity<PlayerSize>,
        param_entity: Entity<OutputParams>,
    ) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new("EzClip", cx));
        let focus_handle = cx.focus_handle();
        Self::listen_open(&param_entity, cx);

        Self {
            title_bar,
            size: size_entity.clone(),
            output_parames: param_entity.clone(),
            player: Player::new(size_entity, param_entity),
            selection_range: (None, None),
            focus_handle,
        }
    }

    pub fn new_player(&mut self) {
        self.player = Player::new(self.size.clone(), self.output_parames.clone());
    }

    pub fn close_file(&mut self) {
        self.selection_range = (None, None);
        self.new_player();
    }

    pub fn run(&mut self, cx: &mut Context<Self>) {
        self.player.start_play(cx);
        cx.notify();
    }

    fn play_percent(&self) -> f32 {
        self.player.play_percentage().unwrap_or(0.)
    }

    fn set_range(&mut self, cx: &mut Context<Self>, percent_range: (Option<f32>, Option<f32>)) {
        if let Some(a) = percent_range.0 {
            self.selection_range.0 = Some(a);
        }
        if let Some(b) = percent_range.1 {
            self.selection_range.1 = Some(b);
        }

        self.output_parames.update(cx, |p, _| {
            p.selected_range = self.get_sec_range();
        });
    }

    fn get_sec_range(&self) -> Option<(f32, f32)> {
        if self.selection_range.0.is_some() && self.selection_range.1.is_some() {
            if let Some(dur) = self.player.duration_sec() {
                return Some((
                    (self.selection_range.0.unwrap() as f64 * dur) as f32,
                    (self.selection_range.1.unwrap() as f64 * dur) as f32,
                ));
            }
        }
        None
    }

    fn listen_open(params: &Entity<OutputParams>, cx: &mut Context<Self>) {
        cx.observe(params, |this, e: Entity<OutputParams>, cx| {
            if this.player.is_init() {
                this.close_file();
            }

            if let Some(path) = e.read(cx).path.clone() {
                this.player.open(cx, &path).unwrap();
                this.run(cx);
            }
        })
        .detach();
    }
}

impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.player.get_state() == PlayState::Playing {
            cx.focus_self(window);
            cx.on_next_frame(window, |_, _, cx| {
                cx.notify();
            });
        }

        div()
            .bg(cx.theme().background)
            .v_flex()
            .size_full()
            .child(self.title_bar.clone())
            .child(
                div()
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(on_close_file))
                    .on_action(cx.listener(on_switch))
                    .on_action(cx.listener(on_back))
                    .on_action(cx.listener(on_foward))
                    .on_action(cx.listener(on_set_start))
                    .on_action(cx.listener(on_set_end))
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
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(self.player.view(window)),
                    )
                    .child(
                        // control zone
                        control_area(self, cx),
                    ),
            )
    }
}

fn control_area(this: &mut MyApp, cx: &mut Context<MyApp>) -> AnyElement {
    let play_state = this.player.get_state();
    let weak = cx.weak_entity();

    div()
        .v_flex()
        .w_full()
        .h_1_3()
        .justify_between()
        .border_1()
        .border_color(cx.theme().border)
        .child(div().flex().w_full().child(
            Timeline::new("process", this.play_percent(), this.selection_range).on_click(
                move |pct, cx| {
                    weak.update(cx, |this, _| {
                        this.player.set_playtime(|_, dur| dur * pct as f64);
                    })
                    .unwrap();
                },
            ),
        ))
        .child(
            div()
                .h_flex()
                .justify_between()
                .items_center()
                .w_full()
                .p_4()
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            RoundButton::new("switch_play")
                                .blue()
                                .when_else(
                                    play_state != PlayState::Playing,
                                    |this| this.icon_path(icons::rounded::PLAY_FILLED),
                                    |this| this.icon_path(icons::rounded::PAUSE_FILLED),
                                )
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(SwitchPlay), cx)),
                        )
                        .child(
                            RoundButton::new("go-back")
                                .icon_path(icons::rounded::REPLAY_10_FILLED)
                                .small_icon()
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(Back), cx)),
                        )
                        .child(
                            RoundButton::new("go-forward")
                                .icon_path(icons::rounded::FORWARD_10_FILLED)
                                .small_icon()
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(Forward), cx)),
                        )
                        .child(
                            RoundButton::new("last-key")
                                .icon_path(icons::rounded::FIRST_PAGE_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.player.next_key();
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("next-key")
                                .icon_path(icons::rounded::LAST_PAGE_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.player.next_key();
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("to-beginning")
                                .icon_path(icons::rounded::KEYBOARD_TAB_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(start) = this.selection_range.0 {
                                        this.player.set_playtime(|_, dur| dur * start as f64);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("to-end")
                                .icon_path(icons::rounded::KEYBOARD_TAB_R_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(end) = this.selection_range.1 {
                                        this.player.set_playtime(|_, dur| dur * end as f64);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("set-start")
                                .icon_path(icons::rounded::SELECTED_START_FILLED)
                                .small_icon()
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(SetStart), cx)),
                        )
                        .child(
                            RoundButton::new("set-end")
                                .icon_path(icons::rounded::SELECTED_END_FILLED)
                                .small_icon()
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(SetEnd), cx)),
                        ),
                )
                .when_else(
                    play_state != PlayState::Stopped,
                    |div| {
                        div.child(Chip::new().label(format!(
                            "{} / {}",
                            format_sec(this.player.current_playtime() as f64),
                            format_sec(this.player.duration_sec().unwrap_or(0.))
                        )))
                    },
                    |div| div.child(Chip::new().label("-- : -- / -- : --")),
                ),
        )
        .into_any_element()
}

fn on_close_file(this: &mut MyApp, _: &Close, _: &mut Window, cx: &mut Context<MyApp>) {
    this.close_file();
    cx.notify();
}
fn on_switch(this: &mut MyApp, _: &SwitchPlay, _: &mut Window, cx: &mut Context<MyApp>) {
    match this.player.get_state() {
        PlayState::Playing => this.player.pause_play(),
        PlayState::Paused => this.player.resume_play(),
        PlayState::Stopped => (),
    }
    cx.notify();
}
fn on_back(this: &mut MyApp, _: &Back, _: &mut Window, cx: &mut Context<MyApp>) {
    this.player.set_playtime(|now, _| now - 10.);
    cx.notify();
}
fn on_foward(this: &mut MyApp, _: &Forward, _: &mut Window, cx: &mut Context<MyApp>) {
    this.player.set_playtime(|now, _| now + 10.);
    cx.notify();
}

fn on_set_start(this: &mut MyApp, _: &SetStart, _: &mut Window, cx: &mut Context<MyApp>) {
    if this.player.get_state() != PlayState::Stopped {
        this.set_range(cx, (Some(this.play_percent()), None));
    }
    cx.notify();
}
fn on_set_end(this: &mut MyApp, _: &SetEnd, _: &mut Window, cx: &mut Context<MyApp>) {
    if this.player.get_state() != PlayState::Stopped {
        this.set_range(cx, (None, Some(this.play_percent())));
    }
    cx.notify();
}

fn format_sec(sec: f64) -> String {
    format!(
        "{:02}:{:02}",
        sec.round() as i32 / 60,
        sec.round() as i32 % 60,
    )
}

impl Focusable for MyApp {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
