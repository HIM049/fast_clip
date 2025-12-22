use std::sync::Arc;

use async_channel::Receiver;
use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme, StyledExt, button::Button};

use crate::{
    components::app_title_bar::AppTitleBar,
    ui::{ffmpeg::VideoDecoder, player_size::PlayerSize, viewer::Viewer},
};

pub struct MyApp {
    size: Entity<PlayerSize>,
    title_bar: Entity<AppTitleBar>,
    decoder: VideoDecoder,
    pub frame: Arc<Vec<u8>>,
}

impl MyApp {
    pub fn new(cx: &mut Context<Self>, size_entity: Entity<PlayerSize>) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new("EzClip", cx));
        let frame: Arc<Vec<u8>> = Arc::new(vec![0, 0, 0, 0]);
        Self {
            size: size_entity.clone(),
            title_bar,
            decoder: VideoDecoder::new(size_entity),
            frame,
        }
    }

    pub fn run(&mut self, cx: &mut Context<Self>) {
        self.decoder
            .open(
                cx,
                "D:/Videos/Records/Apex Legends 2024.05.04 - 18.07.10.04.DVR.mp4".into(),
            )
            .unwrap();

        let rx = self.decoder.run();
        self.listen_frame(cx, rx);
        println!("DEBUG: length of returned frame {}", self.frame.len());
        // let buff = RgbaImage::from_raw(vd.width, vd.height, vd.frame).unwrap();

        cx.notify();
    }

    pub fn listen_frame(&self, cx: &mut Context<Self>, rx: Receiver<Arc<Vec<u8>>>) {
        cx.spawn(async move |weak, cx| {
            while let Ok(frame) = rx.recv().await {
                weak.update(cx, |app, cx| {
                    app.frame = frame;
                    cx.notify();
                })
                .unwrap();
            }
        })
        .detach();
    }
}

impl Render for MyApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let v = Viewer::new(cx, self.frame.clone(), self.size.clone());

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
                            .child(v),
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
                            .child(Button::new("run").child("Run").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.run(cx);
                                },
                            ))),
                    ),
            )
    }
}
