use std::{path::PathBuf, sync::Arc};

use gpui::*;
use gpui_component::*;

use crate::ui::{about::AboutView, app::MyApp, player::player_size::PlayerSize};
use reqwest_client;
mod components;
mod ui;

actions!([Quit, About]);

fn main() {
    ffmpeg_next::init().unwrap();

    let http = reqwest_client::ReqwestClient::user_agent(
        format!("Eazycut/{}", env!("CARGO_PKG_VERSION")).as_str(),
    )
    .unwrap();

    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_component::init(cx);
        init_theme(cx);

        let size_entity = cx.new(|_cx| PlayerSize::new());

        cx.set_http_client(Arc::new(http));
        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.), px(700.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Picargo".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
                }),
                show: true,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| MyApp::new(cx, size_entity));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();

        cx.on_action(|_: &About, cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(400.), px(300.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("About".into()),
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
                    let view = cx.new(|_| AboutView);
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .unwrap();
        });

        // cx.spawn(async move |acx| {
        //     acx.open_window(WindowOptions::default(), |window, cx| {
        //         let view = cx.new(|_| MyApp);
        //         // This first level on the window, should be a Root.
        //         cx.new(|cx| Root::new(view, window, cx))
        //     })?;

        //     Ok::<_, anyhow::Error>(())
        // })
        // .detach();
    });
}

fn init_theme(cx: &mut App) {
    let theme_name = SharedString::from("macOS Classic Dark");

    if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme);
        }
    }) {
        println!("error when init theme: {}", err);
    }
}
