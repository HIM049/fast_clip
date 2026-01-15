use app_assets::icons;
use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window, div, prelude::FluentBuilder,
};
use gpui_component::{ActiveTheme, StyledExt};

use crate::{
    Close,
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
                    self.selection_range.0.unwrap() * dur,
                    self.selection_range.1.unwrap() * dur,
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
        cx.focus_self(window);

        if self.player.get_state() == PlayState::Playing {
            cx.on_next_frame(window, |_, _, cx| {
                cx.notify();
            });
        }

        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &Close, _, cx| {
                this.close_file();
                cx.notify();
            }))
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
                        this.player.set_playtime(|_, dur| dur * pct);
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
                            RoundButton::new("button_play")
                                .blue()
                                .when_else(
                                    play_state != PlayState::Playing,
                                    |this| this.icon_path(icons::rounded::PLAY_FILLED),
                                    |this| this.icon_path(icons::rounded::PAUSE_FILLED),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    match this.player.get_state() {
                                        PlayState::Playing => this.player.pause_play(),
                                        PlayState::Paused => this.player.resume_play(),
                                        PlayState::Stopped => (),
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("replay")
                                .icon_path(icons::rounded::REPLAY_10_FILLED)
                                .small_icon()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.player.set_playtime(|now, _| now - 10.);
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("forward")
                                .icon_path(icons::rounded::FORWARD_10_FILLED)
                                .small_icon()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.player.set_playtime(|now, _| now + 10.);
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("last")
                                .icon_path(icons::rounded::SKIP_PREVIOUS_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(start) = this.selection_range.0 {
                                        this.player.set_playtime(|_, dur| dur * start);
                                    }
                                })),
                        )
                        .child(
                            RoundButton::new("next")
                                .icon_path(icons::rounded::SKIP_NEXT_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(end) = this.selection_range.1 {
                                        this.player.set_playtime(|_, dur| dur * end);
                                    }
                                })),
                        )
                        .child(RoundButton::new("a").label("A").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.set_range(cx, (Some(this.play_percent()), None));
                                cx.notify();
                            },
                        )))
                        .child(RoundButton::new("b").label("B").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.set_range(cx, (None, Some(this.play_percent())));
                                cx.notify();
                            },
                        ))),
                )
                .when_else(
                    play_state != PlayState::Stopped,
                    |div| {
                        div.child(Chip::new().label(format!(
                            "{} / {}",
                            format_sec(this.player.current_playtime()),
                            format_sec(this.player.duration_sec().unwrap_or(0.))
                        )))
                    },
                    |div| div.child(Chip::new().label("-- : -- / -- : --")),
                ),
        )
        .into_any_element()
}

fn format_sec(sec: f32) -> String {
    format!(
        "{:02}:{:02}",
        sec.round() as i32 / 60,
        sec.round() as i32 % 60,
    )
}

impl Focusable for MyApp {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
