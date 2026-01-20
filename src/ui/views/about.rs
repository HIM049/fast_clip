use app_assets::icons::rounded;
use gpui::{ImageSource, ParentElement, Render, Resource, Styled, div, img, px};
use gpui_component::{Icon, Sizable, StyledExt, button::Toggle, label::Label};

pub struct AboutView;

impl Render for AboutView {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let icon_source = ImageSource::Resource(Resource::Embedded("app_icon.png".into()));
        let version = env!("CARGO_PKG_VERSION");
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .font_medium()
            .child(
                div()
                    .v_flex()
                    .items_center()
                    .gap_3()
                    .child(img(icon_source).size(px(100.)))
                    .child(
                        div()
                            .v_flex()
                            .items_center()
                            .child(Label::new("Fast Clip").font_bold().text_xl())
                            .child(format!("version {}", version))
                            .child(
                                Toggle::new("toggle1")
                                    .icon(Icon::new(Icon::empty()).path(rounded::GITHUB))
                                    .large()
                                    .on_click(|_, _, cx| {
                                        cx.open_url("https://github.com/HIM049/fast_clip");
                                    }),
                            ),
                    ),
            )
    }
}
