use gpui::{AppContext, Entity, ParentElement, Render, SharedString, Styled, div};
use gpui_component::{
    Sizable, StyledExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputState},
    label::Label,
};
use rfd::FileDialog;

use crate::{models::model::OutputParams, ui::output::output::output};

pub struct OutputView {
    params: Entity<OutputParams>,
    input: Entity<InputState>,
}

impl OutputView {
    pub fn new(
        window: &mut gpui::Window,
        cx: &mut gpui::App,
        params: Entity<OutputParams>,
    ) -> Self {
        Self {
            params,
            input: cx.new(|cx| InputState::new(window, cx).default_value("./output.mp4")),
        }
    }

    pub fn run_output(&self, cx: &mut gpui::App) {
        let param = self.params.read(cx);
        let Some(path) = param.path.as_ref() else {
            println!("DEBUG: error when output: None path");
            return;
        };
        let Some(v_ix) = param.video_stream_ix else {
            println!("DEBUG: error when output: None video_stream_ix");
            return;
        };
        let Some(a_ix) = param.audio_stream_ix else {
            println!("DEBUG: error when output: None audio_stream_ix");
            return;
        };
        let Some(range) = param.selected_range else {
            println!("DEBUG: error when output: None selected_range");
            return;
        };
        output(path, v_ix, a_ix, range).unwrap();
    }
}

impl Render for OutputView {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
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
                                .child(Button::new("select").ghost().label("...").on_click(
                                    cx.listener(|this, _, w, cx| {
                                        let p = FileDialog::new()
                                            .set_can_create_directories(true)
                                            .set_file_name("output.mp4")
                                            .set_directory("/")
                                            .save_file();
                                        if let Some(path) = p {
                                            let p = SharedString::from(
                                                path.to_string_lossy().to_string(),
                                            );
                                            this.input.update(cx, move |i, cx| {
                                                i.set_value(p, w, cx);
                                            });
                                        }
                                    }),
                                )),
                        ),
                    )
                    .child(
                        div()
                            .w_full()
                            // .child(Label::new("Output Path"))
                            .child(
                                Checkbox::new("checkbox")
                                    .label("Copy Stream")
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
                            .label("Cancel")
                            .on_click(|_, w, _| {
                                w.remove_window();
                            }),
                    )
                    .child(
                        Button::new("output")
                            .small()
                            .primary()
                            .label("Output")
                            .on_click(cx.listener(|this, _, w, cx| {
                                this.run_output(cx);
                                w.remove_window();
                            })),
                    ),
            )
    }
}
