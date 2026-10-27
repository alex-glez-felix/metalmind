#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Las filas de pantalla: con `wrap`, `gj`, `gk`, `g0`, `g^`, `gm`, `g$` y
//! la ventana con líneas partidas; sin él, la ventana que se corre a lo
//! ancho (`sidescroll`) y `zh`, `zl`, `zH`, `zL`, `zs`, `ze`. El motor mide
//! como una terminal de 80 columnas, cada carácter una celda, y parte donde
//! la línea llega al borde, como nvim con `wrap` y sin `linebreak`: así se
//! compara con `nvim --clean` el cursor, la primera línea y `leftcol`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use std::sync::Arc;

use metalmind_syntax::{Advances, LineStyle, Screen, ScreenLine, ScreenLines};
use metalmind_vim::{Mode, Viewport};
use pretty_assertions::assert_eq;
use rstest::rstest;
use support::{Editor, State};

/// Una terminal: cada carácter mide una celda y la línea se parte en la
/// celda donde llega al borde.
#[derive(Debug)]
struct Terminal;

impl ScreenLines for Terminal {
    fn line(&self, text: &str, _: Option<&LineStyle>, wrap: Option<f32>) -> Arc<ScreenLine> {
        let advances = Advances::monospace(text, 1.0);
        let breaks: Vec<usize> = match wrap {
            Some(width) => text
                .char_indices()
                .map(|(byte, _)| byte)
                .filter(|&byte| byte > 0 && advances.end(byte) > width)
                .scan(0.0, |left, byte| {
                    let breaks = advances.end(byte) - *left > width;
                    if breaks {
                        *left = advances.x(byte);
                    }
                    Some(breaks.then_some(byte))
                })
                .flatten()
                .collect(),
            None => Vec::new(),
        };
        Arc::new(ScreenLine::with_breaks(advances, breaks))
    }
}

/// `rows` filas de 80 columnas, con la primera línea `top` (desde 1).
#[derive(Clone, Copy, Debug)]
struct Window {
    rows: usize,
    top: usize,
    wrap: bool,
}

const fn wrap(rows: usize, top: usize) -> Window {
    Window {
        rows,
        top,
        wrap: true,
    }
}

const fn nowrap(rows: usize, top: usize) -> Window {
    Window {
        rows,
        top,
        wrap: false,
    }
}

/// El cursor, la primera línea (desde 0) y `leftcol` en el motor.
fn engine(before: &str, keys: &str, window: Window) -> (State, usize, usize) {
    let mut editor = Editor::new(before);
    editor.pane.viewport = Viewport::new(window.rows);
    editor.pane.set_screen(Some(Screen {
        width: 80.0,
        wrap: window.wrap,
        column: 1.0,
        lines: Arc::new(Terminal),
    }));
    let moved = editor.pane.scroll_to(
        window.top - 1,
        &editor.buffer,
        &mut editor.selections,
        &mut editor.doc,
        0,
        Mode::Normal,
    );
    assert!(!moved, "el cursor del caso tiene que verse en la ventana");
    editor
        .pane
        .reveal(&mut editor.buffer, &editor.selections, &mut editor.doc, 0);
    editor.keys(keys);
    let visual = matches!(editor.vim.mode(), Mode::Visual(_));
    let primary = editor.selections.primary();
    let state = State {
        text: editor.cursors(),
        mode: support::mode_name(editor.vim.mode()).to_owned(),
        selection: support::render_visual(
            &editor.buffer,
            visual.then_some(primary.anchor),
            primary.head,
        ),
    };
    let viewport = &editor.pane.viewport;
    (state, viewport.top(), viewport.left())
}

fn nvim(before: &str, keys: &str, window: Window) -> (State, usize, usize) {
    let setup = if window.wrap { "" } else { "set nowrap" };
    let screen = support::Screen {
        rows: window.rows,
        scrolloff: 0,
        top: window.top - 1,
        tall_headings: false,
    };
    support::nvim::run_on_after(setup, before, keys, screen)
}

/// Líneas de largos distintos, varias de más de una fila, hechas de
/// `0123456789` para que se note la columna; la 5 empieza con espacios.
fn note() -> Vec<String> {
    let digits = |len: usize| "0123456789".repeat(len / 10 + 1)[..len].to_owned();
    [30, 200, 75, 170, 0, 90, 12, 250, 40, 160, 5, 81]
        .iter()
        .enumerate()
        .map(|(index, &len)| {
            if index == 4 {
                format!("      {}", digits(120))
            } else {
                digits(len)
            }
        })
        .chain((13..=40).map(|i| format!("{i:03} {}", "x".repeat(i * 13 % 170))))
        .collect()
}

fn marked(cursor: (usize, usize)) -> String {
    let mut lines = note();
    lines[cursor.0 - 1].insert(cursor.1, '|');
    lines.join("\n")
}

#[rstest]
// gj y gk dentro de una línea partida y entre líneas
#[case(wrap(20, 1), (2, 5), "gj")]
#[case(wrap(20, 1), (2, 5), "2gj")]
#[case(wrap(20, 1), (2, 5), "3gj")]
#[case(wrap(20, 1), (2, 5), "5gj")]
#[case(wrap(20, 1), (2, 165), "gk")]
#[case(wrap(20, 1), (2, 165), "3gk")]
#[case(wrap(20, 1), (3, 70), "gk")]
#[case(wrap(20, 1), (3, 70), "gjgj")]
#[case(wrap(20, 1), (4, 100), "gjgjgkgk")]
#[case(wrap(20, 1), (2, 5), "gjj")]
#[case(wrap(20, 1), (2, 5), "$gj")]
#[case(wrap(20, 1), (2, 5), "$gk")]
#[case(wrap(20, 1), (1, 0), "gk")]
#[case(wrap(20, 1), (2, 5), "dgj")]
#[case(wrap(20, 1), (2, 85), "d2gk")]
#[case(wrap(20, 1), (2, 5), "vgjd")]
// g0, g^, gm y g$ en cada fila
#[case(wrap(20, 1), (2, 95), "g0")]
#[case(wrap(20, 1), (2, 95), "g$")]
#[case(wrap(20, 1), (2, 175), "g$")]
#[case(wrap(20, 1), (2, 95), "gm")]
#[case(wrap(20, 1), (2, 175), "gm")]
#[case(wrap(20, 1), (2, 5), "2g$")]
#[case(wrap(20, 1), (2, 95), "g$j")]
#[case(wrap(20, 1), (5, 30), "g^")]
#[case(wrap(20, 1), (2, 95), "dg0")]
#[case(wrap(20, 1), (2, 95), "dg$")]
// la ventana con líneas partidas
#[case(wrap(10, 1), (1, 0), "6j")]
#[case(wrap(10, 1), (1, 0), "15j")]
#[case(wrap(10, 1), (1, 0), "G")]
#[case(wrap(10, 1), (1, 0), "10gj")]
#[case(wrap(10, 1), (1, 0), "<C-d>")]
#[case(wrap(10, 1), (1, 0), "<C-f>")]
#[case(wrap(10, 1), (1, 0), "<C-e>")]
#[case(wrap(10, 6), (8, 0), "zb")]
#[case(wrap(10, 6), (8, 0), "zz")]
#[case(wrap(10, 6), (9, 0), "L")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn con_wrap_como_nvim(#[case] window: Window, #[case] cursor: (usize, usize), #[case] keys: &str) {
    let before = marked(cursor);
    let (engine, engine_top, _) = engine(&before, keys, window);
    let (nvim, nvim_top, _) = nvim(&before, keys, window);
    assert_eq!((engine.text, engine_top), (nvim.text, nvim_top));
}

#[rstest]
// la ventana sigue al cursor a lo ancho
#[case(nowrap(20, 1), (2, 5), "$")]
#[case(nowrap(20, 1), (2, 5), "$0")]
#[case(nowrap(20, 1), (2, 5), "79l")]
#[case(nowrap(20, 1), (2, 5), "80l")]
#[case(nowrap(20, 1), (2, 5), "85l")]
#[case(nowrap(20, 1), (2, 5), "$80h")]
#[case(nowrap(20, 1), (2, 5), "$100h")]
#[case(nowrap(20, 1), (2, 5), "$j")]
#[case(nowrap(20, 1), (2, 5), "$k")]
#[case(nowrap(20, 1), (2, 5), "A01234<Esc>")]
// zh, zl, zH, zL, zs y ze
#[case(nowrap(20, 1), (2, 5), "zl")]
#[case(nowrap(20, 1), (2, 5), "10zl")]
#[case(nowrap(20, 1), (2, 50), "30zl")]
#[case(nowrap(20, 1), (2, 5), "$zh")]
#[case(nowrap(20, 1), (2, 5), "$50zh")]
#[case(nowrap(20, 1), (2, 5), "zL")]
#[case(nowrap(20, 1), (2, 5), "$zH")]
#[case(nowrap(20, 1), (2, 100), "zs")]
#[case(nowrap(20, 1), (2, 100), "ze")]
#[case(nowrap(20, 1), (2, 30), "ze")]
#[case(nowrap(20, 1), (1, 25), "50zl")]
#[case(nowrap(20, 1), (2, 5), "vzlzl")]
// g0, g^, gm, g$, gj y gk sin wrap
#[case(nowrap(20, 1), (2, 100), "zsg0")]
#[case(nowrap(20, 1), (2, 100), "zsg$")]
#[case(nowrap(20, 1), (2, 100), "zsgm")]
#[case(nowrap(20, 1), (2, 5), "g$")]
#[case(nowrap(20, 1), (2, 5), "2g$")]
#[case(nowrap(20, 1), (5, 2), "g^")]
#[case(nowrap(20, 1), (2, 5), "gj")]
#[case(nowrap(20, 1), (2, 5), "dgj")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn sin_wrap_como_nvim(#[case] window: Window, #[case] cursor: (usize, usize), #[case] keys: &str) {
    let before = marked(cursor);
    let engine = engine(&before, keys, window);
    let nvim = nvim(&before, keys, window);
    assert_eq!(
        (engine.0.text, engine.1, engine.2),
        (nvim.0.text, nvim.1, nvim.2)
    );
}

/// Una línea más alta que la ventana: con el cursor abajo en ella, la
/// ventana oculta sus primeras filas, como `w_skipcol` de vim.
#[test]
fn una_linea_mas_alta_que_la_ventana_oculta_sus_filas_de_arriba() {
    let before = format!("{}|x", "a".repeat(799));
    let (_, top, _) = engine(&before, "", wrap(5, 1));
    assert_eq!(top, 0);
    let mut editor = Editor::new(&before);
    editor.pane.viewport = Viewport::new(5);
    editor.pane.set_screen(Some(Screen {
        width: 80.0,
        wrap: true,
        column: 1.0,
        lines: Arc::new(Terminal),
    }));
    editor
        .pane
        .reveal(&mut editor.buffer, &editor.selections, &mut editor.doc, 0);
    assert_eq!(editor.pane.viewport.skip(), 5, "la fila 10 es la última");
    editor.keys("0");
    assert_eq!(editor.pane.viewport.skip(), 0);
    editor.keys("4gj");
    assert_eq!(editor.pane.viewport.skip(), 0, "la fila 5 cabe");
    editor.keys("gj");
    assert_eq!(editor.pane.viewport.skip(), 1);
    editor.keys("gk");
    assert_eq!(editor.pane.viewport.skip(), 1, "no se mueve si se ve");
}
