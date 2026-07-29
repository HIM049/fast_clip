#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use std::{env, path::PathBuf, sync::Arc};

use gpui::*;
use gpui_component::*;
use rust_i18n::t;

use crate::{
    components::app_menu::{About, Open, Output, Quit, Settings},
    config::AppConfig,
    models::model::{OutputParams, WindowState},
    ui::{
        player::size::PlayerSize,
        views::{self, app::MyApp, settings::SettingsView},
    },
};
use reqwest_client;
mod components;
mod config;
mod models;
mod ui;
mod update;

rust_i18n::i18n!("locales", fallback = "en");

actions!([
    Back, Forward, SwitchPlay, ToRangeA, ToRangeB, SetStart, SetEnd, VolumeUp, VolumeDown
]);

#[cfg(target_os = "macos")]
static OUTPUT_KEY: &str = "cmd-s";
#[cfg(not(target_os = "macos"))]
static OUTPUT_KEY: &str = "ctrl-s";

fn main() {
    ffmpeg_next::init().unwrap();

    let http = reqwest_client::ReqwestClient::user_agent(
        format!("Fastclip/{}", env!("CARGO_PKG_VERSION")).as_str(),
    )
    .unwrap();

    let app = gpui_platform::application().with_assets(app_assets::Assets);
    app.run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_component::init(cx);
        init_theme(cx);
        bind_keys(cx);

        let config = config::load();
        rust_i18n::set_locale(&config.language.as_locale());
        cx.set_global(config);

        // let config_entity: Entity<AppConfig> = cx.new(|_| config);

        let size_entity = cx.new(|_cx| PlayerSize::new());
        let params_entity: Entity<OutputParams> = cx.new(|_| OutputParams::default());
        let window_state = cx.new(|_| WindowState::default());

        cx.set_http_client(Arc::new(http));
        let app_window: AnyWindowHandle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1000.), px(800.)),
                        cx,
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Fast Clip".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
                    }),
                    show: true,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| {
                        cx.on_release(|_, cx| {
                            cx.quit();
                        })
                        .detach();
                        MyApp::new(cx, size_entity, params_entity.clone())
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .unwrap()
            .into();

        cx.on_action(|_: &Quit, cx| {
            cx.quit();
        });
        cx.on_action(open_settings_window(window_state.clone()));
        cx.on_action(open_about_dialog(app_window.clone()));
        cx.on_action(open_output_dialog(
            app_window.clone(),
            params_entity.clone(),
        ));
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
                    params.update(cx, |p, cx| {
                        p.path = Some(path);
                        cx.notify();
                    });
                }
            })
            .detach();
        });

        if cx.global::<AppConfig>().check_update {
            let app_window = app_window.clone();
            let http_client = cx.http_client().clone();
            cx.spawn(
                async move |cx| match update::check_update(http_client).await {
                    Ok(Some(url)) => update::show_update_dialog(app_window, url, cx),
                    Ok(None) => {}
                    Err(error) => eprintln!("failed to check for updates: {error}"),
                },
            )
            .detach();
        }
    });
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("space", SwitchPlay, None)]);
    cx.bind_keys([KeyBinding::new("left", Back, None)]);
    cx.bind_keys([KeyBinding::new("right", Forward, None)]);
    cx.bind_keys([KeyBinding::new("[", SetStart, None)]);
    cx.bind_keys([KeyBinding::new("]", SetEnd, None)]);
    cx.bind_keys([KeyBinding::new("up", VolumeUp, None)]);
    cx.bind_keys([KeyBinding::new("down", VolumeDown, None)]);
    cx.bind_keys([KeyBinding::new(OUTPUT_KEY, Output, None)]);
}

fn open_settings_window(window_state: Entity<WindowState>) -> impl Fn(&Settings, &mut App) {
    move |_: &Settings, cx| {
        window_state.update(cx, |ws, cx| {
            if active_window(cx, &mut ws.settings_handle).is_ok() {
                return;
            }

            let window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(800.), px(600.)),
                cx,
            )));
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds,
                        titlebar: Some(TitlebarOptions {
                            title: Some(t!("menu.application.settings").into()),
                            appears_transparent: false,
                            traffic_light_position: None,
                        }),
                        focus: true,
                        show: true,
                        ..Default::default()
                    },
                    |window, cx| {
                        let view = cx.new(|_| SettingsView::new());
                        cx.new(|cx| Root::new(view, window, cx))
                    },
                )
                .unwrap();
            ws.settings_handle = Some(handle);
        });
    }
}

fn open_output_dialog(
    window: AnyWindowHandle,
    params: Entity<OutputParams>,
) -> impl Fn(&Output, &mut App) {
    move |_: &Output, cx: &mut App| {
        if !params.read(cx).all_some() {
            return;
        }
        let params = params.clone();
        cx.defer(move |cx| {
            cx.update_window(window, move |_, w, cx| {
                if w.has_active_dialog(cx) {
                    return;
                }

                let view = cx.new(|cx| views::output::OutputView::new(w, cx, params));
                w.open_dialog(cx, move |dialog, _, cx| {
                    views::output::build_output_dialog(dialog, view.clone(), cx)
                });
            })
            .unwrap();
        });
    }
}

fn open_about_dialog(window: AnyWindowHandle) -> impl Fn(&About, &mut App) {
    move |_: &About, cx: &mut App| {
        cx.defer(move |cx| {
            cx.update_window(window, move |_, w, cx| {
                w.open_dialog(cx, move |dialog, _, _| views::about::build_about(dialog));
            })
            .unwrap();
        });
    }
}

fn init_theme(cx: &mut App) {
    let theme_name = SharedString::from("macOS Classic Dark");

    if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
        if let Some(theme_cfg) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            let theme = Theme::global_mut(cx);
            theme.apply_config(&theme_cfg);
            theme.notification.placement = Anchor::TopCenter;
            theme.notification.max_items = 1;
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
