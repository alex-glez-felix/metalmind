//! Raíz de la ventana: aloja la vista elegida, mide cada frame, pinta el
//! overlay y conduce el benchmark.

use std::time::{Duration, Instant};

use gpui_kit::{
    Action, AnyView, App, AppContext as _, Context, Entity, IntoElement, KeyBinding,
    ParentElement as _, Render, Styled as _, TitlebarOptions, Window, WindowOptions, canvas, div,
};

use crate::cli::View;
use crate::editor::VimEditor;
use crate::hud::{Status, hud};
use crate::kit::KitEditor;
use crate::session::Session;
use crate::tokens::Tokens;

const BENCH_DURATION: Duration = Duration::from_secs(10);
const BENCH_LINES_PER_FRAME: u32 = 2;

#[derive(Clone, PartialEq, Eq, Default, Debug, Action)]
#[action(namespace = app, name = "quit")]
pub struct Quit;

pub fn bind_keys(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
}

enum Content {
    Vim(Entity<VimEditor>),
    Kit(Entity<KitEditor>),
}

impl Content {
    fn view(&self) -> AnyView {
        match self {
            Self::Vim(editor) => editor.clone().into(),
            Self::Kit(editor) => editor.clone().into(),
        }
    }

    fn status(&self, cx: &App) -> Status {
        match self {
            Self::Vim(editor) => editor.read(cx).status(),
            Self::Kit(editor) => editor.read(cx).status(cx),
        }
    }

    fn bench_step(&self, window: &mut Window, cx: &mut App) {
        match self {
            Self::Vim(editor) => editor.update(cx, |editor, cx| {
                editor.scroll_lines(f64::from(BENCH_LINES_PER_FRAME), cx);
            }),
            Self::Kit(editor) => {
                editor
                    .read(cx)
                    .clone()
                    .move_down(BENCH_LINES_PER_FRAME, window, cx);
            }
        }
    }
}

enum Bench {
    Off,
    /// Espera el primer frame para no medir el arranque.
    Waiting,
    Running(Instant),
}

pub struct Shell {
    content: Content,
    bench: Bench,
}

/// La vista vim es la raíz de su ventana: el `Root` de gpui-kit liga `tab`,
/// `shift-tab` y `ctrl-c`, y esos bindings ganan a los listeners de tecla.
///
/// # Errors
/// Si la plataforma no puede abrir la ventana.
pub fn open(
    view: View,
    text: &str,
    read: Duration,
    bench: bool,
    cx: &mut App,
) -> gpui_kit::Result<()> {
    let options = window_options(view, bench);
    let bench = if bench { Bench::Waiting } else { Bench::Off };
    match view {
        View::Vim => {
            cx.open_window(options, |window, cx| {
                let built = Instant::now();
                let editor = cx.new(|cx| VimEditor::new(text, cx));
                let content = Content::Vim(editor.clone());
                record_load(&content, read + built.elapsed(), cx);
                let focus_handle = editor.read(cx).focus_handle().clone();
                window.focus(&focus_handle, cx);
                cx.new(|_| Shell::new(content, bench))
            })?;
        }
        View::Kit => {
            gpui_kit::open_window(options, cx, |window, cx| {
                let built = Instant::now();
                let editor = cx.new(|cx| KitEditor::new(text, window, cx));
                let content = Content::Kit(editor.clone());
                record_load(&content, read + built.elapsed(), cx);
                editor.read(cx).clone().focus(window, cx);
                cx.new(|_| Shell::new(content, bench))
            })?;
        }
    }
    Ok(())
}

fn record_load(content: &Content, load: Duration, cx: &mut App) {
    let lines = content.status(cx).lines;
    let session = cx.global_mut::<Session>();
    session.metrics.record_load(load);
    session.info.lines = lines;
}

/// En el benchmark GPUI no baja a 30 fps si la ventana pierde el foco: se
/// mide el render, no la política de ahorro de energía.
fn window_options(view: View, bench: bool) -> WindowOptions {
    let defaults = WindowOptions::default();
    WindowOptions {
        inactive_frame_interval: defaults.inactive_frame_interval.filter(|_| !bench),
        titlebar: Some(TitlebarOptions {
            title: Some(format!("nota spike · {}", view.name()).into()),
            ..TitlebarOptions::default()
        }),
        app_id: Some("nota-spike-gpui".to_owned()),
        ..defaults
    }
}

impl Shell {
    fn new(content: Content, bench: Bench) -> Self {
        Self { content, bench }
    }

    fn drive_bench(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let started = match self.bench {
            Bench::Off => return,
            Bench::Waiting if cx.global::<Session>().metrics.first_frame().is_none() => {
                window.request_animation_frame();
                return;
            }
            Bench::Waiting => {
                cx.global_mut::<Session>().metrics.reset_frames();
                let now = Instant::now();
                self.bench = Bench::Running(now);
                now
            }
            Bench::Running(started) => started,
        };
        if started.elapsed() >= BENCH_DURATION {
            self.bench = Bench::Off;
            window.on_next_frame(|_, cx| cx.quit());
            return;
        }
        self.content.bench_step(window, cx);
        window.request_animation_frame();
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = Instant::now();
        cx.global_mut::<Session>().metrics.frame_started(started);
        self.drive_bench(window, cx);
        let tokens = Tokens::from_app(cx);
        let status = self.content.status(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(tokens.background)
            .child(div().flex_1().min_h_0().child(self.content.view()))
            .child(hud(status, cx.global::<Session>(), &tokens))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, (), _, cx| {
                        cx.global_mut::<Session>()
                            .metrics
                            .frame_painted(started, Instant::now());
                    },
                )
                .absolute()
                .size_0(),
            )
    }
}
