use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Decorations, Entity, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Render, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, WindowControlArea, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, InteractiveElementExt, Sizable, h_flex, menu::AppMenuBar,
};

use crate::components::app_menu::{self, init};

pub const TITLE_BAR_HEIGHT: Pixels = px(34.);
#[cfg(target_os = "macos")]
const TITLE_BAR_LEFT_PADDING: Pixels = px(80.);
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_LEFT_PADDING: Pixels = px(12.);

pub struct AppTitleBar {
    app_menu: Entity<AppMenuBar>,
    child: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
}

impl AppTitleBar {
    pub fn new(title: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            app_menu: app_menu::init(title, cx),
            child: Rc::new(|_, _| div().into_any_element()),
        }
    }

    pub fn child<F, E>(mut self, f: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.child = Rc::new(move |window, cx| f(window, cx).into_any_element());
        self
    }
}

struct TitleBarState {
    should_move: bool,
}

impl Render for AppTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });

        div()
            .id("title-bar")
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .h(TITLE_BAR_HEIGHT)
            .pl(TITLE_BAR_LEFT_PADDING)
            .border_b_1()
            .border_color(cx.theme().title_bar_border)
            .bg(cx.theme().title_bar)
            .when(cfg!(target_os = "linux"), |this| {
                this.on_double_click(|_, window, _| window.zoom_window())
            })
            .when(cfg!(target_os = "macos"), |this| {
                this.on_double_click(|_, window, _| window.titlebar_double_click())
            })
            .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                state.should_move = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }),
            )
            .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                if state.should_move {
                    state.should_move = false;
                    window.start_window_move();
                }
            }))
            .child(
                h_flex()
                    .id("bar")
                    .window_control_area(gpui::WindowControlArea::Drag)
                    .h_full()
                    .justify_between()
                    .flex_shrink_0()
                    .flex_1()
                    .child(self.app_menu.clone()), // .when(cfg!(target_os = "windows"), |this| {
                                                   //     this.window_control_area(gpui::WindowControlArea::Min)
                                                   // }),
            )
            .child(
                h_flex()
                    .id("window-controls")
                    .items_center()
                    .flex_shrink_0()
                    .h_full()
                    .child(ControlIcon::Minimize)
                    .child(if window.is_maximized() {
                        ControlIcon::Restore
                    } else {
                        ControlIcon::Maximize
                    })
                    .child(ControlIcon::Close {
                        on_close_window: None,
                    }),
            )
    }
}

#[derive(IntoElement, Clone)]
enum ControlIcon {
    Minimize,
    Restore,
    Maximize,
    Close {
        on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>,
    },
}

impl ControlIcon {
    fn minimize() -> Self {
        Self::Minimize
    }

    fn restore() -> Self {
        Self::Restore
    }

    fn maximize() -> Self {
        Self::Maximize
    }

    fn close(on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>) -> Self {
        Self::Close { on_close_window }
    }

    fn id(&self) -> &'static str {
        match self {
            Self::Minimize => "minimize",
            Self::Restore => "restore",
            Self::Maximize => "maximize",
            Self::Close { .. } => "close",
        }
    }

    fn icon(&self) -> IconName {
        match self {
            Self::Minimize => IconName::WindowMinimize,
            Self::Restore => IconName::WindowRestore,
            Self::Maximize => IconName::WindowMaximize,
            Self::Close { .. } => IconName::WindowClose,
        }
    }

    fn window_control_area(&self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Restore | Self::Maximize => WindowControlArea::Max,
            Self::Close { .. } => WindowControlArea::Close,
        }
    }

    fn is_close(&self) -> bool {
        matches!(self, Self::Close { .. })
    }

    #[inline]
    fn hover_fg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_foreground
        } else {
            cx.theme().secondary_foreground
        }
    }

    #[inline]
    fn hover_bg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger
        } else {
            cx.theme().secondary_hover
        }
    }

    #[inline]
    fn active_bg(&self, cx: &mut App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_active
        } else {
            cx.theme().secondary_active
        }
    }
}

impl RenderOnce for ControlIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_linux = cfg!(target_os = "linux");
        let is_windows = cfg!(target_os = "windows");
        let hover_fg = self.hover_fg(cx);
        let hover_bg = self.hover_bg(cx);
        let active_bg = self.active_bg(cx);
        let icon = self.clone();
        let on_close_window = match &self {
            ControlIcon::Close { on_close_window } => on_close_window.clone(),
            _ => None,
        };

        div()
            .id(self.id())
            .flex()
            .w(TITLE_BAR_HEIGHT)
            .h_full()
            .flex_shrink_0()
            .justify_center()
            .content_center()
            .items_center()
            .text_color(cx.theme().foreground)
            .hover(|style| style.bg(hover_bg).text_color(hover_fg))
            .active(|style| style.bg(active_bg).text_color(hover_fg))
            .when(is_windows, |this| {
                this.window_control_area(self.window_control_area())
            })
            .when(is_linux, |this| {
                this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    match icon {
                        Self::Minimize => window.minimize_window(),
                        Self::Restore | Self::Maximize => window.zoom_window(),
                        Self::Close { .. } => {
                            if let Some(f) = on_close_window.clone() {
                                f(&ClickEvent::default(), window, cx);
                            } else {
                                window.remove_window();
                            }
                        }
                    }
                })
            })
            .child(Icon::new(self.icon()).small())
    }
}
