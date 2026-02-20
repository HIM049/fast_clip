use std::{ops::Range, path::PathBuf, time::Duration};

use app_assets::icons::{self, rounded};
use gpui::{
    AnyElement, AppContext, Context, Entity, ExternalPaths, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Task, Window, div,
    prelude::FluentBuilder, px, rgba, svg,
};
use gpui_component::{ActiveTheme, Colorize, StyledExt};

use crate::{
    Back, Forward, SetEnd, SetStart, SwitchPlay, VolumeDown, VolumeUp,
    components::{
        app_menu::{Close, OpenPlayerSetting},
        app_title_bar::AppTitleBar,
    },
    models::model::OutputParams,
    ui::{
        button::RoundButton,
        chip::Chip,
        player::{
            player::{PlayState, Player},
            size::PlayerSize,
        },
        timeline::Timeline,
        views::player_settings::{PlayerSettings, PlayerSettingsView},
    },
};

#[derive(Debug)]
enum MessageState {
    Timer { _task: Task<()> },
    Seeking,
    None,
}

pub struct MyApp {
    title_bar: Entity<AppTitleBar>,
    size: Entity<PlayerSize>,
    output_parames: Entity<OutputParams>,
    player: Player,
    // here selection_range is percentage of progress
    selection_range: Range<Option<f32>>,
    focus_handle: FocusHandle,
    settings: Entity<PlayerSettings>,
    message: Option<String>,
    message_icon: Option<String>,
    message_mgr: MessageState,
}

impl MyApp {
    pub fn new(
        cx: &mut Context<Self>,
        size_entity: Entity<PlayerSize>,
        param_entity: Entity<OutputParams>,
    ) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new("Fast Clip", cx));
        let focus_handle = cx.focus_handle();
        let settings = cx.new(|_| PlayerSettings::default());
        Self::listen_open(&param_entity, cx);
        Self::listen_settings(&settings, cx);

        Self {
            title_bar,
            size: size_entity.clone(),
            output_parames: param_entity.clone(),
            player: Player::new(size_entity, param_entity),
            selection_range: Range {
                start: None,
                end: None,
            },
            focus_handle,
            settings: settings,
            message: None,
            message_icon: None,
            message_mgr: MessageState::None,
        }
    }

    /// handle open and play file
    pub fn open_file(&mut self, cx: &mut Context<Self>, path: &PathBuf) {
        if self.player.is_init() {
            self.close_file(cx);
        }
        self.player.open(cx, &path).unwrap();
        self.player.start_play(cx, None);

        // init settings params
        let params = self.output_parames.read(cx);
        if let (Some(audio_ix), Some(audio_rails)) =
            (params.audio_stream_ix, params.audio_rails.clone())
        {
            self.settings.update(cx, |s, _| {
                s.audio_ix = audio_ix;
                s.audio_rails = audio_rails;
            });
        }
        cx.notify();
    }

    /// close file and reset player
    pub fn close_file(&mut self, cx: &mut Context<Self>) {
        self.selection_range = Range {
            start: None,
            end: None,
        };
        self.output_parames.update(cx, |p, _| {
            p.selected_range = None;
        });
        self.player = Player::new(self.size.clone(), self.output_parames.clone());
    }

    /// reselect audio rail
    fn reselect_rail(&mut self, cx: &mut Context<Self>, ix: usize) {
        // save current time
        self.player.pause_play();
        let time = self.player.current_playtime();
        // reset decoder
        self.close_file(cx);
        if let Some(p) = self.output_parames.read(cx).path.clone() {
            self.player.open(cx, &p).unwrap();
            self.player.start_play(cx, Some(ix));
        }
        // back to time before
        if self.player.get_state() == PlayState::Playing {
            self.player.seek_to(time);
        }
    }

    /// calc player percent
    fn play_percent(&self) -> f32 {
        self.player.play_percentage().unwrap_or(0.)
    }

    fn active_range(&self) -> Option<Range<f32>> {
        if self.selection_range.start.is_some() || self.selection_range.end.is_some() {
            let start = self.selection_range.start.unwrap_or(0.);
            let end = self.selection_range.end.unwrap_or(1.);
            Some(Range { start, end })
        } else {
            None
        }
    }

    /// calc selected range as sec
    fn range_time(&self) -> Option<Range<f64>> {
        if let (Some(dur), Some(pct_range)) = (self.player.duration_sec(), self.active_range())
            && pct_range.start < pct_range.end
        {
            let start = dur * pct_range.start as f64;
            let end = dur * pct_range.end as f64;
            return Some(Range { start, end });
        }
        None
    }

    /// set and update range
    fn update_range(&mut self, cx: &mut Context<Self>, percent_range: (Option<f32>, Option<f32>)) {
        if let Some(a) = percent_range.0 {
            self.selection_range.start = Some(a);
        }
        if let Some(b) = percent_range.1 {
            self.selection_range.end = Some(b);
        }
        self.output_parames.update(cx, |p, _| {
            p.selected_range = self.range_time();
        });
    }

    fn show_message(
        &mut self,
        cx: &mut Context<Self>,
        message: String,
        icon: Option<String>,
        dur: Option<Duration>,
    ) {
        if let Some(dur) = dur {
            let t = cx.spawn(async move |weak, cx| {
                cx.background_executor().timer(dur).await;
                weak.update(cx, |this, _| {
                    this.message = None;
                    this.message_icon = None;
                    this.message_mgr = MessageState::None;
                })
                .unwrap();
            });
            self.message_mgr = MessageState::Timer { _task: t };
        } else {
            self.message_mgr = MessageState::None;
        }
        self.message = Some(message);
        self.message_icon = icon.into();
    }

    fn show_vol(&mut self, cx: &mut Context<Self>) {
        let gain = self.player.get_gain();
        let icon: String = if gain == 0.0 {
            rounded::VOLUME_MUTE
        } else if gain <= 0.6 {
            rounded::VOLUME_DOWN
        } else {
            rounded::VOLUME_UP
        }
        .into();
        self.show_message(
            cx,
            format!("{:3.0}%", gain * 100.),
            Some(icon),
            Some(Duration::from_secs(2)),
        );
    }

    /// listen open file event
    fn listen_open(params: &Entity<OutputParams>, cx: &mut Context<Self>) {
        cx.observe(params, |this, e: Entity<OutputParams>, cx| {
            if let Some(path) = e.read(cx).path.clone() {
                this.open_file(cx, &path);
            }
        })
        .detach();
    }

    fn listen_settings(params: &Entity<PlayerSettings>, cx: &mut Context<Self>) {
        cx.observe(params, |this: &mut MyApp, e: Entity<PlayerSettings>, cx| {
            this.reselect_rail(cx, e.read(cx).audio_ix);
        })
        .detach();
    }
}

impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = cx.theme().background.darken(0.5);
        if self.player.get_state() != PlayState::Stopped {
            cx.focus_self(window);
            cx.on_next_frame(window, |_, _, cx| {
                cx.notify();
            });
        }
        if matches!(self.message_mgr, MessageState::None)
            || matches!(self.message_mgr, MessageState::Seeking)
        {
            if self.player.is_seeking() {
                self.message = Some("Loading...".into());
                self.message_icon = None;
                self.message_mgr = MessageState::Seeking;
            } else {
                self.message = None;
                self.message_mgr = MessageState::None;
            }
        }

        div()
            .bg(cx.theme().background)
            .v_flex()
            .size_full()
            .child(self.title_bar.clone())
            .child(
                div()
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(on_open_settings))
                    .on_action(cx.listener(on_close_file))
                    .on_action(cx.listener(on_switch))
                    .on_action(cx.listener(on_back))
                    .on_action(cx.listener(on_foward))
                    .on_action(cx.listener(on_set_start))
                    .on_action(cx.listener(on_set_end))
                    .on_action(cx.listener(on_vol_up))
                    .on_action(cx.listener(on_vol_down))
                    .on_drop(cx.listener(|this, e: &ExternalPaths, _, cx| {
                        if let Some(path) = e.paths().first() {
                            this.open_file(cx, path);
                        }
                    }))
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
                            .bg(bg_color)
                            .child(self.player.view(window))
                            .when_some(self.message.clone(), |this, msg| {
                                this.child(message_box(msg, self.message_icon.clone()))
                            }),
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
    let bg_color = cx.theme().background;

    div()
        .w_full()
        .border_1()
        .border_color(cx.theme().border)
        .bg(bg_color)
        .child(
            div().flex().w_full().child(
                Timeline::new(
                    "timeline",
                    this.play_percent(),
                    this.selection_range.clone(),
                )
                .on_click(move |pct, cx| {
                    weak.update(cx, |this, _| {
                        this.player.seek_player(|_, dur| dur * pct as f64);
                    })
                    .unwrap();
                }),
            ),
        )
        .child(
            div()
                .h_flex()
                .justify_between()
                .items_center()
                .w_full()
                .p_4()
                .pt_1()
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
                                .icon_path(icons::rounded::REPLAY_5_FILLED)
                                .small_icon()
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(Back), cx)),
                        )
                        .child(
                            RoundButton::new("go-forward")
                                .icon_path(icons::rounded::FORWARD_5_FILLED)
                                .small_icon()
                                .on_click(|_, w, cx| w.dispatch_action(Box::new(Forward), cx)),
                        )
                        .child(
                            RoundButton::new("last-key")
                                .icon_path(icons::rounded::FIRST_PAGE_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.player.last_key();
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
                                    if let Some(start) = this.selection_range.start {
                                        this.player.seek_player(|_, dur| dur * start as f64);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            RoundButton::new("to-end")
                                .icon_path(icons::rounded::KEYBOARD_TAB_R_FILLED)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(end) = this.selection_range.end {
                                        this.player.seek_player(|_, dur| dur * end as f64);
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
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .when_some(this.range_time(), |this, time| {
                            this.child(Chip::new().border().label(format!(
                                "A {} -> B {}",
                                format_sec(time.start),
                                format_sec(time.end)
                            )))
                        })
                        .when_else(
                            play_state != PlayState::Stopped,
                            |d| {
                                d.child(
                                    div()
                                        .h_flex()
                                        .rounded_full()
                                        .bg(rgba(0xf0e59a26))
                                        .when_some(this.range_time(), |this, time| {
                                            this.child(
                                                div()
                                                    .h_flex()
                                                    .pl_4()
                                                    .pr_2()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        svg()
                                                            .path(rounded::TIME_DURATION)
                                                            .text_color(gpui::white())
                                                            .size_5(),
                                                    )
                                                    .child(format_sec(
                                                        (time.end - time.start).max(0.),
                                                    )),
                                            )
                                        })
                                        .child(Chip::new().border().bold().label(format!(
                                            "{} / {}",
                                            format_sec(this.player.current_playtime() as f64),
                                            format_sec(this.player.duration_sec().unwrap_or(0.))
                                        ))),
                                )
                            },
                            |div| div.child(Chip::new().border().bold().label("-- : -- / -- : --")),
                        ),
                ),
        )
        .into_any_element()
}

fn message_box(msg: impl IntoElement, icon: Option<String>) -> AnyElement {
    div()
        .h_flex()
        .justify_center()
        .items_center()
        .min_w(px(170.))
        .absolute()
        .border_1()
        .border_color(gpui::white())
        .bg(gpui::black().alpha(0.7))
        .rounded_sm()
        .px_10()
        .py_6()
        .font_bold()
        .gap_2()
        .when_some(icon, |this, icon| {
            this.justify_between()
                .child(svg().path(icon).size_8().text_color(gpui::white()))
        })
        .child(msg)
        .into_any_element()
}

fn on_open_settings(
    this: &mut MyApp,
    _: &OpenPlayerSetting,
    _: &mut Window,
    cx: &mut Context<MyApp>,
) {
    PlayerSettingsView::open_window(cx, this.settings.clone()).unwrap();
    cx.notify();
}
fn on_close_file(this: &mut MyApp, _: &Close, _: &mut Window, cx: &mut Context<MyApp>) {
    this.close_file(cx);
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
    this.player.seek_player(|now, _| now - 5.);
    cx.notify();
}
fn on_foward(this: &mut MyApp, _: &Forward, _: &mut Window, cx: &mut Context<MyApp>) {
    this.player.seek_player(|now, _| now + 5.);
    cx.notify();
}

fn on_set_start(this: &mut MyApp, _: &SetStart, _: &mut Window, cx: &mut Context<MyApp>) {
    if this.player.get_state() != PlayState::Stopped {
        this.update_range(cx, (Some(this.play_percent()), None));
    }
    cx.notify();
}
fn on_set_end(this: &mut MyApp, _: &SetEnd, _: &mut Window, cx: &mut Context<MyApp>) {
    if this.player.get_state() != PlayState::Stopped {
        this.update_range(cx, (None, Some(this.play_percent())));
    }
    cx.notify();
}
fn on_vol_up(this: &mut MyApp, _: &VolumeUp, _: &mut Window, cx: &mut Context<MyApp>) {
    let gain = this.player.get_gain() + 0.1;
    this.player.set_gain(gain);
    this.show_vol(cx);
    cx.notify();
}
fn on_vol_down(this: &mut MyApp, _: &VolumeDown, _: &mut Window, cx: &mut Context<MyApp>) {
    let gain = this.player.get_gain() - 0.1;
    this.player.set_gain(gain);
    this.show_vol(cx);
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
