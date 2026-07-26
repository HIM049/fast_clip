use gpui::{Action, App, Entity, Menu, MenuItem, OwnedMenu, OwnedMenuItem, SharedString, actions};
use gpui_component::{GlobalState, Theme, menu::AppMenuBar};
use rust_i18n::t;

use crate::config::{self as config_store, AppConfig, Language};

actions!(menu, [Quit, About, Open, Close, Output, OpenPlayerSetting]);

#[derive(Action, Clone, PartialEq, Eq)]
#[action(namespace = menu, no_json)]
pub struct SelectLocale(pub SharedString);

pub fn init(
    title: impl Into<SharedString>,
    config: Entity<AppConfig>,
    cx: &mut App,
) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    update_app_menu(title.clone(), app_menu_bar.clone(), cx);

    cx.on_action({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        let config = config.clone();
        move |s: &SelectLocale, cx: &mut App| {
            let locale = s.0.as_str();
            rust_i18n::set_locale(locale);
            if let Some(language) = Language::from_locale(locale) {
                config.update(cx, |app_config, _| {
                    app_config.language = language;
                    if let Err(err) = config_store::save(app_config) {
                        println!("failed to save config: {}", err);
                    }
                });
            }
            update_app_menu(title.clone(), app_menu_bar.clone(), cx);
        }
    });

    // Observe theme changes to update the menu to refresh the checked state
    cx.observe_global::<Theme>({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        move |cx| {
            update_app_menu(title.clone(), app_menu_bar.clone(), cx);
        }
    })
    .detach();

    app_menu_bar
}

fn update_app_menu(title: impl Into<SharedString>, app_menu_bar: Entity<AppMenuBar>, cx: &mut App) {
    // let mode = cx.theme().mode;

    let menus = vec![
        Menu {
            name: title.into(),
            disabled: false,
            items: vec![
                MenuItem::action(t!("menu.about"), About),
                MenuItem::Separator,
                MenuItem::action(t!("menu.quit"), Quit),
            ],
        }
        .owned(),
        Menu {
            name: SharedString::from(t!("menu.file")),
            disabled: false,
            items: vec![
                MenuItem::action(t!("menu.open"), Open),
                MenuItem::action(t!("menu.close"), Close),
                MenuItem::Separator,
                MenuItem::action(t!("menu.output"), Output),
            ],
        }
        .owned(),
        Menu {
            name: SharedString::from(t!("menu.player")),
            disabled: false,
            items: vec![MenuItem::action(t!("menu.audio"), OpenPlayerSetting)],
        }
        .owned(),
        Menu {
            name: SharedString::from(t!("menu.settings")),
            disabled: false,
            items: vec![language_menu()],
        }
        .owned(),
    ];

    GlobalState::global_mut(cx).set_app_menus(menus);
    // cx.set_menus();

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    })
}

fn language_menu() -> MenuItem {
    let locale = rust_i18n::locale().to_string();
    MenuItem::Submenu(Menu {
        name: SharedString::from(t!("menu.language")),
        disabled: false,
        items: vec![
            MenuItem::action("English", SelectLocale(Language::En.as_locale().into()))
                .checked(locale == Language::En.as_locale()),
            MenuItem::action("简体中文", SelectLocale(Language::ZhCn.as_locale().into()))
                .checked(locale == Language::ZhCn.as_locale()),
        ],
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
