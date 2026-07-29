use gpui::{App, BorrowAppContext, Context, IntoElement, Render, SharedString, px};
use gpui_component::setting::{
    NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings,
};
use rust_i18n::t;
use strum::IntoEnumIterator;

use crate::config::{AppConfig, GpuPolicy, StepMode};

pub struct SettingsView;

impl SettingsView {
    pub fn new() -> Self {
        Self
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        Settings::new("app-settings")
            .sidebar_width(px(100.))
            .pages(vec![
                SettingPage::new(text("settings.general"))
                    .default_open(true)
                    .group(
                        SettingGroup::new()
                            .title(text("settings.groups.application"))
                            .items(build_general_group()),
                    )
                    .group(
                        SettingGroup::new()
                            .title(text("settings.groups.player"))
                            .items(build_player_group()),
                    )
                    .group(
                        SettingGroup::new()
                            .title(text("settings.groups.control"))
                            .items(build_control_group()),
                    ),
            ])
    }
}

fn build_general_group() -> Vec<SettingItem> {
    vec![
        SettingItem::new(
            text("settings.check_update.title"),
            SettingField::switch(
                move |cx: &App| cx.global::<AppConfig>().check_update,
                move |enabled: bool, cx: &mut App| {
                    cx.update_global(|g: &mut AppConfig, _| {
                        g.check_update = enabled;
                        g.save();
                    });
                },
            ),
        )
        .description(text("settings.check_update.description")),
    ]
}

fn build_player_group() -> Vec<SettingItem> {
    vec![
        SettingItem::new(
            text("settings.gpu_policy.title"),
            SettingField::dropdown(
                GpuPolicy::iter()
                    .map(|policy| (policy.value().into(), text(policy.i18n_key())))
                    .collect(),
                move |cx: &App| cx.global::<AppConfig>().gpu_policy.value().into(),
                {
                    move |gpu_policy: SharedString, cx: &mut App| {
                        let Some(gpu_policy) = GpuPolicy::from_value(gpu_policy.as_ref()) else {
                            return;
                        };
                        cx.update_global(|g: &mut AppConfig, _| {
                            g.gpu_policy = gpu_policy;
                            g.save();
                        });
                    }
                },
            ),
        )
        .description(text("settings.gpu_policy.description")),
    ]
}

fn build_control_group() -> Vec<SettingItem> {
    vec![
        SettingItem::new(
            text("settings.seek_mode.title"),
            SettingField::dropdown(
                vec![
                    (
                        StepMode::Percent.value().into(),
                        text("settings.seek_mode.percent"),
                    ),
                    (
                        StepMode::Second.value().into(),
                        text("settings.seek_mode.seconds"),
                    ),
                ],
                move |cx: &App| cx.global::<AppConfig>().step_mode.value().into(),
                move |step_mode: SharedString, cx: &mut App| {
                    let Some(step_mode) = StepMode::from_value(step_mode.as_ref()) else {
                        return;
                    };
                    cx.update_global(|g: &mut AppConfig, _| {
                        g.step_mode = step_mode;
                        g.save();
                    });
                },
            ),
        )
        .description(text("settings.seek_mode.description")),
        SettingItem::new(
            text("settings.seek_percent.title"),
            SettingField::number_input(
                NumberFieldOptions {
                    min: 0.1,
                    max: 100.0,
                    step: 1.0,
                },
                move |cx: &App| cx.global::<AppConfig>().step_percent * 100.,
                {
                    move |step: f64, cx: &mut App| {
                        cx.update_global(|g: &mut AppConfig, _| {
                            g.step_percent = step / 100.;
                            g.save();
                        });
                    }
                },
            ),
        )
        .description(text("settings.seek_percent.description")),
        SettingItem::new(
            text("settings.seek_seconds.title"),
            SettingField::number_input(
                NumberFieldOptions {
                    min: 0.1,
                    max: 600.0,
                    step: 0.1,
                },
                move |cx: &App| cx.global::<AppConfig>().step_sec,
                move |step: f64, cx: &mut App| {
                    cx.update_global(|g: &mut AppConfig, _| {
                        g.step_sec = step;
                        g.save();
                    });
                },
            ),
        )
        .description(text("settings.seek_seconds.description")),
        // SettingItem::new(
        //     "Working directory",
        //     SettingField::input(
        //         move |cx: &App| directory_view.read(cx).working_directory.clone(),
        //         {
        //             let view = view.clone();
        //             move |directory: SharedString, cx: &mut App| {
        //                 view.update(cx, |settings, cx| {
        //                     settings.working_directory = directory;
        //                     cx.notify();
        //                 });
        //             }
        //         },
        //     ),
        // )
        // .description("Used only as a temporary example value."),
    ]
}

fn text(key: &str) -> SharedString {
    t!(key).to_string().into()
}
