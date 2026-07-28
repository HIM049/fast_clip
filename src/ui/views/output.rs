use std::path::{Path, PathBuf};

use gpui::{AppContext, ClickEvent, Context, Entity, ParentElement, Render, Styled, Window, div};
use gpui_component::{
    Disableable, IndexPath, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::Dialog,
    input::{Input, InputState},
    label::Label,
    select::{Select, SelectState},
};
use path_absolutize::Absolutize;
use rust_i18n::t;

use crate::{
    models::model::OutputParams,
    ui::{output::output::output, player::model::AudioRail},
};

pub struct OutputView {
    params: Entity<OutputParams>,
    input: Entity<InputState>,
    output_path: PathBuf,
    audio_select: Entity<SelectState<Vec<AudioRail>>>,
    update_path: bool,
}

impl OutputView {
    pub fn new(
        window: &mut gpui::Window,
        cx: &mut gpui::App,
        params: Entity<OutputParams>,
    ) -> Self {
        let p = params.read(cx);
        let rails = p.audio_rails.clone().unwrap();
        let list_ix = rails
            .iter()
            .position(|r| r.ix == p.audio_stream_ix.unwrap());
        let selected_index = if let Some(ix) = list_ix {
            Some(IndexPath::new(ix))
        } else {
            None
        };
        let audio_select = cx.new(|cx| SelectState::new(rails, selected_index, window, cx));

        let path = params.read(cx).path.clone().unwrap();
        let new_path = if let Some(stem) = path.file_stem() {
            let mut new_name = stem.to_string_lossy().into_owned();
            new_name.push_str("_edit");

            if let Some(ext) = path.extension() {
                new_name.push('.');
                new_name.push_str(&ext.to_string_lossy());
            }

            path.with_file_name(new_name)
        } else {
            path.with_file_name("output.mp4")
        };

        let default = new_path
            .absolutize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let input = cx.new(|cx| InputState::new(window, cx).default_value(default));
        Self {
            params,
            input,
            output_path: new_path,
            audio_select,
            update_path: false,
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
        let range = param.selected_range.as_ref().unwrap();
        if let Some(ix) = self.audio_select.read(cx).selected_value() {
            a_ix = *ix;
        }
        if let Err(e) = output(path, &self.output_path, v_ix, a_ix, range) {
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
                    this.output_path = path;
                    this.update_path = true;
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
        if self.update_path {
            self.input.update(cx, |i, cx| {
                let path = self.output_path.to_string_lossy().into_owned();
                i.set_value(path, w, cx);
            });
            self.update_path = false;
        }

        div().w_full().v_flex().gap_3().child(
            div()
                .flex()
                .v_flex()
                .gap_3()
                .child(
                    div().w_full().child(Label::new(t!("output.path"))).child(
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
                        .child(Label::new(t!("output.audio_track")))
                        .child(Select::new(&self.audio_select)),
                )
                .child(
                    div()
                        .w_full()
                        // .child(Label::new("Output Path"))
                        .child(
                            Checkbox::new("checkbox")
                                .label(t!("output.copy_stream").to_string())
                                .checked(true)
                                .disabled(true),
                        ),
                ),
        )
    }
}

pub fn build_output_dialog(dialog: Dialog, output_view: Entity<OutputView>) -> Dialog {
    let output = output_view.clone();
    dialog
        .title(t!("output.title"))
        .overlay_closable(false)
        .child(output_view)
        .footer(
            div()
                .h_flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("cancel")
                        .label(t!("common.actions.cancel"))
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .child(
                    Button::new("output")
                        .primary()
                        .label(t!("output.title"))
                        .on_click(move |_, window, cx| {
                            output.update(cx, |view, cx| view.run_output(cx));
                            window.close_dialog(cx);
                        }),
                ),
        )
}
