//! Una ventana GPUI vacía con lo mismo que enlaza el visor (GPUI y
//! `env_logger`): la base para medir cuánto suma la pizarra al binario.
//!
//! `cargo build --release --example vacio`

use gpui_kit::{App, AppContext, Context, IntoElement, Render, Styled, Window, WindowOptions, div};

struct Vacia;

impl Render for Vacia {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn main() {
    env_logger::init();
    gpui_kit::application().run(|cx: &mut App| {
        if let Err(error) = cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Vacia)) {
            eprintln!("no se pudo abrir la ventana: {error}");
            cx.quit();
        }
    });
}
