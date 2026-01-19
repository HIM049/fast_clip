use std::path::{Path, PathBuf};

use gpui::{AppContext, ClickEvent, Context, Entity, ParentElement, Render, Styled, Window, div};
use gpui_component::{
    IndexPath, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputState},
    label::Label,
    select::{Select, SelectState},
};
use rust_i18n::t;

use crate::{
    models::model::OutputParams,
    ui::{output::output::output, player::ffmpeg::AudioRail},
};

pub struct OutputView {
    params: Entity<OutputParams>,
    input: Entity<InputState>,
    updated_path: Option<PathBuf>,
    audio_select: Entity<SelectState<Vec<AudioRail>>>,
}

impl OutputView {
    pub fn new(
        window: &mut gpui::Window,
        cx: &mut gpui::App,
        params: Entity<OutputParams>,
    ) -> Self {
        let p = params.read(cx);
        let rails = p.audio_rails.clone().unwrap();
        let selected_index = Some(IndexPath::new(p.audio_stream_ix.unwrap()));
        let audio_select = cx.new(|cx| SelectState::new(rails, selected_index, window, cx));
        Self {
            params,
            input: cx.new(|cx| InputState::new(window, cx).default_value("./output.mp4")),
            updated_path: None,
            audio_select,
        }
    }

    pub fn run_output(&self, cx: &mut gpui::App) {
        let param = self.params.read(cx);
        if !param.all_some() {
            return;
        }
        let path = param.path.as_ref().unwrap();
        let v_ix = param.video_stream_ix.unwrap();
        let mut a_ix = param.audio_stream_ix.unwrap();
        let range = param.selected_range.unwrap();
        if let Some(ix) = self.audio_select.read(cx).selected_value() {
            a_ix = *ix;
        }
        if let Err(e) = output(path, v_ix, a_ix, range) {
            println!("error when output: {}", e);
        }
    }

    fn listen_path(_: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let result = cx.prompt_for_new_path(Path::new("./"), Some("ouput.mp4"));

        cx.spawn(async |this, cx| {
            let Ok(r) = result.await else {
                return;
            };
            let Ok(r) = r else {
                return;
            };
            if let Some(path) = r {
                this.update(cx, |this, _| {
                    this.updated_path = Some(path);
                })
                .unwrap();
            }
        })
        .detach();
    }
}

impl Render for OutputView {
    fn render(
        &mut self,
        w: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        if let Some(path) = self.updated_path.take() {
            self.input.update(cx, |i, cx| {
                self.params.update(cx, |p, _| {
                    p.path = Some(path.clone());
                });
                i.set_value(path.to_string_lossy().to_string(), w, cx);
            });
        }

        div()
            .size_full()
            .flex()
            .v_flex()
            .p_3()
            .justify_between()
            .child(
                div()
                    .flex()
                    .v_flex()
                    .gap_3()
                    .child(
                        div().w_full().child(Label::new("Output Path")).child(
                            div()
                                .w_full()
                                .flex()
                                .h_flex()
                                .child(Input::new(&self.input))
                                .child(
                                    Button::new("select")
                                        .ghost()
                                        .label("...")
                                        .on_click(cx.listener(Self::listen_path)),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .child(Label::new("Audio Rail"))
                            .child(Select::new(&self.audio_select)),
                    )
                    .child(
                        div()
                            .w_full()
                            // .child(Label::new("Output Path"))
                            .child(
                                Checkbox::new("checkbox")
                                    .label(t!("ui.cp-stream").to_string())
                                    .checked(true)
                                    .on_click(|_, _, _| {}),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel")
                            .small()
                            .label(t!("ui.cancel"))
                            .on_click(|_, w, _| {
                                w.remove_window();
                            }),
                    )
                    .child(
                        Button::new("output")
                            .small()
                            .primary()
                            .label(t!("ui.output"))
                            .on_click(cx.listener(|this, _, w, cx| {
                                this.run_output(cx);
                                w.remove_window();
                            })),
                    ),
            )
    }
}
