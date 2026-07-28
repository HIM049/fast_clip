use app_assets::icons::rounded;
use gpui::{ImageSource, ParentElement, Resource, Styled, div, img, px};
use gpui_component::{
    Icon, StyledExt,
    button::{Button, ButtonVariants},
    dialog::Dialog,
    label::Label,
};
use rust_i18n::t;

pub fn build_about(dialog: Dialog) -> Dialog {
    let icon_source = ImageSource::Resource(Resource::Embedded("app_icon.png".into()));
    let version = env!("CARGO_PKG_VERSION");
    dialog
        .title(t!("menu.application.about"))
        .child(
            div()
                .h_flex()
                .justify_center()
                .items_center()
                .my_8()
                .gap_10()
                .child(img(icon_source).size(px(100.)))
                .child(
                    div()
                        .v_flex()
                        .justify_center()
                        .items_start()
                        .child(Label::new("Fast Clip").font_bold().text_xl())
                        .child(format!("v{}", version)),
                ),
        )
        .footer(
            div().h_flex().justify_end().items_center().child(
                Button::new("github")
                    .label("Github")
                    .primary()
                    .icon(Icon::new(Icon::empty()).path(rounded::GITHUB))
                    .on_click(|_, _, cx| cx.open_url("https://github.com/HIM049/fast_clip")),
            ),
        )
}
