use gpui::{App, Context, IntoElement, Render, SharedString, px};
use gpui_component::setting::{
    NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings,
};

pub struct SettingsView {
    check_update: bool,
    gpu_policy: SharedString,
    seek_step_seconds: f64,
    working_directory: SharedString,
}

impl SettingsView {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            check_update: true,
            gpu_policy: "Standard".into(),
            seek_step_seconds: 5.0,
            working_directory: String::new().into(),
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let notification_view = view.clone();
        let density_view = view.clone();
        let seek_step_view = view.clone();
        let directory_view = view.clone();

        Settings::new("app-settings")
            .sidebar_width(px(100.))
            .pages(vec![SettingPage::new("General").default_open(true).group(
                SettingGroup::new().items(vec![
                        SettingItem::new(
                            "Check Update",
                            SettingField::switch(
                                move |cx: &App| notification_view.read(cx).check_update,
                                {
                                    let view = view.clone();
                                    move |enabled: bool, cx: &mut App| {
                                        view.update(cx, |settings, cx| {
                                            settings.check_update = enabled;
                                            cx.notify();
                                        });
                                    }
                                },
                            ),
                        )
                        .description("Remind when a new update is available."),
                        SettingItem::new(
                            "GPU Policy",
                            SettingField::dropdown(
                                vec![
                                    ("Software Only".into(), "Software Only".into()),
                                    ("Prefer Integrated GPU".into(), "integrated".into()),
                                    ("Prefer Discrete GPU".into(), "discrete".into()),
                                ],
                                move |cx: &App| density_view.read(cx).gpu_policy.clone(),
                                {
                                    let view = view.clone();
                                    move |density: SharedString, cx: &mut App| {
                                        view.update(cx, |settings, cx| {
                                            settings.gpu_policy = density;
                                            cx.notify();
                                        });
                                    }
                                },
                            ),
                        )
                        .description("Choose the policy of GPU useage."),
                        SettingItem::new(
                            "Seek Step",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 1.0,
                                    max: 600.0,
                                    step: 1.0,
                                },
                                move |cx: &App| seek_step_view.read(cx).seek_step_seconds,
                                {
                                    let view = view.clone();
                                    move |step: f64, cx: &mut App| {
                                        view.update(cx, |settings, cx| {
                                            settings.seek_step_seconds = step;
                                            cx.notify();
                                        });
                                    }
                                },
                            ),
                        )
                        .description("Seconds to skip when seeking forward or backward."),
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
                    ]),
            )])
    }
}
