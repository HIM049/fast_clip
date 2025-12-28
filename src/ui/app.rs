use std::sync::Arc;

use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme, StyledExt, button::Button};

use crate::{
    components::app_title_bar::AppTitleBar,
    ui::player::{player::Player, player_size::PlayerSize},
};

pub struct MyApp {
    size: Entity<PlayerSize>,
    title_bar: Entity<AppTitleBar>,
    player: Player,
    pub frame: Arc<Vec<u8>>,
    is_playing: bool,
}

impl MyApp {
    pub fn new(cx: &mut Context<Self>, size_entity: Entity<PlayerSize>) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new("EzClip", cx));
        let frame: Arc<Vec<u8>> = Arc::new(vec![0, 0, 0, 0]);

        Self {
            size: size_entity.clone(),
            title_bar,
            player: Player::new(size_entity),
            frame,
            is_playing: false,
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        println!("DEBUG: length of returned frame {}", self.frame.len());
        self.player.open(cx);
    }

    pub fn run(&mut self, cx: &mut Context<Self>) {
        println!("DEBUG: length of returned frame {}", self.frame.len());
        self.is_playing = true;
        self.player.start_play(cx);

        cx.notify();
    }
}

impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        cx.on_next_frame(window, |this, _, cx| {
            if this.is_playing {
                cx.notify();
            }
        });

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
                            .child(self.player.view()),
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
                            .child(Button::new("open").child("Open").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.open(cx);
                                },
                            )))
                            .child(Button::new("run").child("Run").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.run(cx);
                                },
                            ))),
                    ),
            )
    }
}
