use gpui::{Action, App, BorrowAppContext, Entity, Menu, MenuItem, SharedString, actions};
use gpui_component::{GlobalState, Theme, menu::AppMenuBar};
use rust_i18n::t;

use crate::config::{self, AppConfig, Language};
use crate::ui::player::settings::PlayerSettings;

actions!(
    menu,
    [
        Quit,
        Settings,
        About,
        Open,
        Close,
        Output,
        ClearSelectedRange
    ]
);

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = menu, no_json)]
pub struct SelectLocale(pub SharedString);

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = menu, no_json)]
pub struct SelectAudioRail(pub usize);

pub fn init(
    cx: &mut App,
    title: impl Into<SharedString>,
    player_settings: Entity<PlayerSettings>,
) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    update_app_menu(
        title.clone(),
        app_menu_bar.clone(),
        cx,
        player_settings.clone(),
    );

    cx.on_action({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        let p_settings = player_settings.clone();
        move |s: &SelectLocale, cx: &mut App| {
            let locale = s.0.as_str();
            rust_i18n::set_locale(locale);
            if let Some(language) = Language::from_locale(locale) {
                cx.update_global(|g: &mut AppConfig, _| {
                    g.language = language;
                    // TODO: handle auto save in config
                    if let Err(err) = config::save(g) {
                        println!("failed to save config: {}", err);
                    }
                });
            }

            update_app_menu(title.clone(), app_menu_bar.clone(), cx, p_settings.clone());
        }
    });

    cx.on_action({
        let p_settings = player_settings.clone();
        move |s: &SelectAudioRail, cx: &mut App| {
            let new_ix = s.0;
            p_settings.update(cx, |s, cx| {
                if s.audio_ix != new_ix {
                    s.audio_ix = new_ix;
                    cx.notify();
                }
            });
        }
    });

    // Observe theme changes to update the menu to refresh the checked state
    cx.observe_global::<Theme>({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        let p_settings = player_settings.clone();
        move |cx| {
            update_app_menu(title.clone(), app_menu_bar.clone(), cx, p_settings.clone());
        }
    })
    .detach();

    let app_menu = app_menu_bar.clone();
    cx.observe(&player_settings, move |settings, cx| {
        update_app_menu(title.clone(), app_menu.clone(), cx, settings);
    })
    .detach();

    app_menu_bar
}

fn build_menus(
    title: impl Into<SharedString>,
    cx: &App,
    player_settings: Entity<PlayerSettings>,
) -> Vec<Menu> {
    vec![
        Menu {
            name: title.into(),
            disabled: false,
            items: vec![
                MenuItem::action(t!("menu.application.about"), About),
                MenuItem::action(t!("menu.application.settings"), Settings),
                language_menu(),
                MenuItem::Separator,
                MenuItem::action(t!("menu.application.quit"), Quit),
            ],
        },
        Menu {
            name: SharedString::from(t!("menu.file.title")),
            disabled: false,
            items: vec![
                MenuItem::action(t!("menu.file.open"), Open),
                MenuItem::action(t!("menu.file.close"), Close),
                MenuItem::Separator,
                MenuItem::action(t!("menu.file.export"), Output),
            ],
        },
        Menu {
            name: SharedString::from(t!("menu.player.title")),
            disabled: false,
            items: vec![audio_rails_menu(cx, player_settings)],
        },
        Menu {
            name: SharedString::from(t!("menu.editor.title")),
            disabled: false,
            items: vec![MenuItem::action(
                t!("menu.editor.clear_selected_range"),
                ClearSelectedRange,
            )],
        },
    ]
}

fn update_app_menu(
    title: impl Into<SharedString>,
    app_menu_bar: Entity<AppMenuBar>,
    cx: &mut App,
    player_settings: Entity<PlayerSettings>,
) {
    // let mode = cx.theme().mode;

    let title: SharedString = title.into();
    cx.set_menus(build_menus(title.clone(), cx, player_settings.clone()));

    let owned_menus = build_menus(title, cx, player_settings)
        .into_iter()
        .map(|m| m.owned())
        .collect();
    GlobalState::global_mut(cx).set_app_menus(owned_menus);

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    })
}

fn language_menu() -> MenuItem {
    let locale = rust_i18n::locale().to_string();
    MenuItem::Submenu(Menu {
        name: SharedString::from(t!("menu.application.language")),
        disabled: false,
        items: vec![
            MenuItem::action("English", SelectLocale(Language::En.as_locale().into()))
                .checked(locale == Language::En.as_locale()),
            MenuItem::action("简体中文", SelectLocale(Language::ZhCn.as_locale().into()))
                .checked(locale == Language::ZhCn.as_locale()),
        ],
    })
}

fn audio_rails_menu(cx: &App, player_settings: Entity<PlayerSettings>) -> MenuItem {
    let settings = player_settings.read(cx);
    let mut items = vec![];
    for (i, s) in settings.audio_rails.iter().enumerate() {
        let item = MenuItem::action(
            format!(
                "Rail_{} ({})",
                i,
                s.handler_name
                    .clone()
                    .unwrap_or(SharedString::from(t!("menu.player.unnamed_rail")))
            ),
            SelectAudioRail(s.ix),
        )
        .checked(settings.audio_ix == s.ix);

        items.push(item);
    }

    let length = items.len();

    MenuItem::submenu(Menu {
        name: SharedString::from(t!("player_settings.audio_track")),
        items,
        disabled: length == 0,
    })
}

// fn theme_menu(cx: &App) -> MenuItem {
//     let themes = ThemeRegistry::global(cx).sorted_themes();
//     let current_name = cx.theme().theme_name();
//     MenuItem::Submenu(Menu {
//         name: "Theme".into(),
//         items: themes
//             .iter()
//             .map(|theme| {
//                 let checked = current_name == &theme.name;
//                 MenuItem::action(theme.name.clone(), SwitchTheme(theme.name.clone()))
//                     .checked(checked)
//             })
//             .collect(),
//     })
// }
