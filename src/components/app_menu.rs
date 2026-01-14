use gpui::{App, Entity, Menu, MenuItem, SharedString};
use gpui_component::{Theme, menu::AppMenuBar};
use rust_i18n::t;

use crate::{About, Close, Open, Output, Quit, models::model::SelectLocale};

pub fn init(title: impl Into<SharedString>, cx: &mut App) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    update_app_menu(title.clone(), app_menu_bar.clone(), cx);

    cx.on_action({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        move |s: &SelectLocale, cx: &mut App| {
            rust_i18n::set_locale(&s.0.as_str());
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
    cx.set_menus(vec![
        Menu {
            name: title.into(),
            items: vec![
                MenuItem::action(t!("menu.about"), About),
                MenuItem::Separator,
                MenuItem::action(t!("menu.quit"), Quit),
            ],
        },
        Menu {
            name: SharedString::from(t!("menu.file")),
            items: vec![
                MenuItem::action(t!("menu.open"), Open),
                MenuItem::action(t!("menu.close"), Close),
                MenuItem::Separator,
                MenuItem::action(t!("menu.output"), Output),
            ],
        },
        Menu {
            name: SharedString::from(t!("menu.player")),
            items: vec![MenuItem::action(t!("menu.audio"), Quit)],
        },
        Menu {
            name: SharedString::from(t!("menu.settings")),
            items: vec![language_menu()],
        },
    ]);

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    })
}

fn language_menu() -> MenuItem {
    let locale = rust_i18n::locale().to_string();
    MenuItem::Submenu(Menu {
        name: SharedString::from(t!("menu.language")),
        items: vec![
            MenuItem::action("English", SelectLocale("en".into())).checked(locale == "en"),
            MenuItem::action("简体中文", SelectLocale("zh-CN".into())).checked(locale == "zh-CN"),
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
