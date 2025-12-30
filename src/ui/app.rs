use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::{ActiveTheme, StyledExt, button::Button};

use crate::{
    components::app_title_bar::AppTitleBar,
    ui::player::{
        player::{PlayState, Player},
        player_size::PlayerSize,
    },
};

pub struct MyApp {
    title_bar: Entity<AppTitleBar>,
    size: Entity<PlayerSize>,
    player: Player,
}

impl MyApp {
    pub fn new(cx: &mut Context<Self>, size_entity: Entity<PlayerSize>) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new("EzClip", cx));

        Self {
            title_bar,
            size: size_entity.clone(),
            player: Player::new(size_entity),
        }
    }

    pub fn new_player(&mut self) {
        self.player = Player::new(self.size.clone());
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.player.open(cx);
    }

    pub fn run(&mut self, cx: &mut Context<Self>) {
        self.player.start_play(cx);
        cx.notify();
    }
}

impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        cx.on_next_frame(window, |this, _, cx| {
            if this.player.get_state() == PlayState::Playing {
                cx.notify();
            }
        });
        let play_state = self.player.get_state();

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
                            .child(self.player.view(window)),
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
                            .gap_2()
                            .when(play_state == PlayState::Stopped, |this| {
                                this.child(Button::new("open").child("Open").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.open(cx);
                                    },
                                )))
                                .child(
                                    Button::new("run").child("Run").on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.run(cx);
                                        },
                                    )),
                                )
                            })
                            .when(play_state == PlayState::Playing, |this| {
                                this.child(Button::new("pause").child("Pause").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.player.pause_play();
                                        cx.notify();
                                    }),
                                ))
                            })
                            .when(play_state == PlayState::Paused, |this| {
                                this.child(Button::new("resume").child("Resume").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.player.resume_play();
                                        cx.notify();
                                    }),
                                ))
                            })
                            .when(play_state != PlayState::Stopped, |this| {
                                this.child(Button::new("stop").child("Stop").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.new_player();
                                        cx.notify()
                                    },
                                )))
                                .child(Button::new("min").child("-10s").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.player.set_playtime(|now| 0f32.max(now - 10.));
                                        cx.notify();
                                    },
                                )))
                                .child(
                                    Button::new("plus").child("+10s").on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.player.set_playtime(|now| now + 10.);
                                            cx.notify();
                                        },
                                    )),
                                )
                            }),
                    ),
            )
    }
}
