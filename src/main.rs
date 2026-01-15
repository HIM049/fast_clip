use std::{path::PathBuf, sync::Arc};

use gpui::*;
use gpui_component::*;

use crate::{
    models::model::{OutputParams, WindowState},
    ui::{
        player::size::PlayerSize,
        views::{about::AboutView, app::MyApp, output::OutputView},
    },
};
use reqwest_client;
mod components;
mod models;
mod ui;

rust_i18n::i18n!("locales", fallback = "en");

actions!(app, [Quit, About, Open, Close, Output, SelectLocale]);

fn main() {
    ffmpeg_next::init().unwrap();

    let http = reqwest_client::ReqwestClient::user_agent(
        format!("Fastclip/{}", env!("CARGO_PKG_VERSION")).as_str(),
    )
    .unwrap();

    let app = Application::new().with_assets(app_assets::Assets);
    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_component::init(cx);
        init_theme(cx);

        let size_entity = cx.new(|_cx| PlayerSize::new());
        let params_entity: Entity<OutputParams> = cx.new(|_| OutputParams::default());
        let window_state = cx.new(|_| WindowState::default());

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
                    title: Some("FastClip".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
                }),
                show: true,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| MyApp::new(cx, size_entity, params_entity.clone()));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();

        cx.on_action(open_about_window(window_state.clone()));
        cx.on_action(open_output_window(window_state, params_entity.clone()));
        cx.on_action(move |_: &Open, cx| {
            let result = cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: None,
            });

            let params = params_entity.clone();
            cx.spawn(async move |cx: &mut AsyncApp| {
                let Ok(r) = result.await else {
                    return;
                };
                let Ok(r) = r else {
                    return;
                };
                if let Some(paths) = r {
                    println!("DEBUG: got some path: {:?}", paths);
                    let path = paths[0].clone();
                    params
                        .update(cx, |p, cx| {
                            p.path = Some(path);
                            cx.notify();
                        })
                        .unwrap();
                }
            })
            .detach();
        });
    });
}

fn open_output_window(
    window_state: Entity<WindowState>,
    params: Entity<OutputParams>,
) -> impl Fn(&Output, &mut App) {
    move |_: &Output, cx: &mut App| {
        window_state.update(cx, |ws, cx| {
            match active_window(cx, &mut ws.output_handle) {
                Ok(_) => return,
                Err(_) => (),
            }

            let window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(500.), px(300.)),
                cx,
            )));
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds,
                        titlebar: Some(TitlebarOptions {
                            title: Some("Output".into()),
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
                        let view = cx.new(|cx| OutputView::new(window, cx, params.clone()));
                        cx.new(|cx| Root::new(view, window, cx))
                    },
                )
                .unwrap();
            ws.output_handle = Some(handle);
        });
    }
}

fn open_about_window(window_state: Entity<WindowState>) -> impl Fn(&About, &mut App) {
    move |_: &About, cx: &mut App| {
        window_state.update(cx, |ws, cx| {
            match active_window(cx, &mut ws.about_handle) {
                Ok(_) => return,
                Err(_) => (),
            }

            let window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(400.), px(300.)),
                cx,
            )));

            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds,
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
            ws.about_handle = Some(handle)
        });
    }
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

fn active_window(cx: &mut App, win_handle: &mut Option<WindowHandle<Root>>) -> Result<(), ()> {
    if let Some(wh) = win_handle {
        if let Some(active) = wh.is_active(cx) {
            if active {
                return Ok(());
            } else {
                wh.update(cx, |_, w, _| {
                    w.activate_window();
                })
                .unwrap();
                return Ok(());
            }
        } else {
            *win_handle = None;
        }
    }
    Err(())
}
