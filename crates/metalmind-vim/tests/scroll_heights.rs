#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! La ventana con headings más altos que el texto. En el motor un `#` mide
//! tres filas y un `##` dos ([`support::TALL_HEADINGS`]); en nvim, esas
//! líneas son tan largas que `wrap` las parte en tres y en dos filas de 80
//! columnas. Así el port de `move.c` con `plines` se compara con nvim: el
//! cursor y la primera línea visible tienen que dar lo mismo.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use pretty_assertions::assert_eq;
use rstest::rstest;
use support::Screen;

/// 100 líneas: un `#` cada diez y un `##` cinco más abajo.
fn note(cursor: (usize, usize)) -> String {
    let mut lines: Vec<String> = (1..=100)
        .map(|i| match i % 10 {
            1 => format!("# {i:03} {}", "h".repeat(200)),
            6 => format!("## {i:03} {}", "s".repeat(120)),
            _ => format!("{i:03} texto"),
        })
        .collect();
    lines[cursor.0 - 1].insert(cursor.1, '|');
    lines.join("\n")
}

const fn screen(rows: usize, scrolloff: usize, top: usize) -> Screen {
    Screen {
        rows,
        scrolloff,
        top: top - 1,
        tall_headings: true,
    }
}

#[rstest]
// mover el cursor
#[case(screen(10, 0, 1), (1, 0), "5j")]
#[case(screen(10, 0, 1), (1, 0), "8j")]
#[case(screen(10, 0, 1), (1, 0), "15j")]
#[case(screen(10, 0, 1), (1, 0), "40j")]
#[case(screen(10, 0, 1), (1, 0), "G")]
#[case(screen(10, 0, 91), (100, 0), "gg")]
#[case(screen(10, 0, 31), (33, 0), "5k")]
#[case(screen(10, 3, 1), (1, 0), "6j")]
#[case(screen(10, 3, 1), (1, 0), "12j")]
#[case(screen(10, 3, 31), (34, 0), "4k")]
#[case(screen(10, 3, 1), (1, 0), "50G")]
#[case(screen(12, 2, 21), (26, 0), "jjjjjj")]
// zt, zz y zb, en un heading y en texto
#[case(screen(10, 0, 21), (26, 0), "zt")]
#[case(screen(10, 0, 21), (26, 0), "zz")]
#[case(screen(10, 0, 21), (26, 0), "zb")]
#[case(screen(10, 0, 21), (24, 0), "zb")]
#[case(screen(10, 0, 21), (24, 0), "zz")]
#[case(screen(10, 3, 21), (24, 0), "zt")]
#[case(screen(10, 3, 21), (24, 0), "zb")]
#[case(screen(10, 0, 21), (25, 0), "41zt")]
#[case(screen(10, 0, 21), (25, 0), "41zb")]
// H, M y L
#[case(screen(10, 0, 21), (24, 0), "H")]
#[case(screen(10, 0, 21), (24, 0), "M")]
#[case(screen(10, 0, 21), (24, 0), "L")]
#[case(screen(10, 0, 18), (19, 0), "M")]
#[case(screen(10, 0, 21), (24, 0), "3H")]
#[case(screen(10, 0, 21), (24, 0), "2L")]
#[case(screen(10, 3, 21), (24, 0), "H")]
#[case(screen(10, 3, 21), (24, 0), "L")]
// ctrl-e y ctrl-y
#[case(screen(10, 0, 21), (24, 0), "<C-e>")]
#[case(screen(10, 0, 21), (24, 0), "5<C-e>")]
#[case(screen(10, 0, 21), (24, 0), "<C-y>")]
#[case(screen(10, 0, 21), (26, 0), "3<C-y>")]
#[case(screen(10, 3, 21), (24, 0), "<C-e>")]
#[case(screen(10, 3, 21), (24, 0), "<C-y>")]
// ctrl-d, ctrl-u, ctrl-f y ctrl-b
#[case(screen(10, 0, 1), (1, 0), "<C-d>")]
#[case(screen(10, 0, 41), (43, 0), "<C-u>")]
#[case(screen(10, 0, 41), (43, 0), "<C-b>")]
#[case(screen(10, 0, 1), (1, 0), "2<C-f>")]
#[case(screen(10, 3, 21), (24, 0), "<C-d>")]
#[case(screen(10, 3, 41), (43, 0), "<C-u>")]
#[case(screen(10, 3, 21), (24, 0), "<C-f>")]
#[case(screen(10, 3, 41), (43, 0), "<C-b>")]
#[case(screen(10, 0, 91), (95, 0), "<C-d>")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn the_window_moves_like_nvim_with_wrapped_lines(
    #[case] screen: Screen,
    #[case] cursor: (usize, usize),
    #[case] keys: &str,
) {
    let before = note(cursor);
    let (engine, engine_top) = support::run_engine_on(&before, keys, Some(screen));
    let (nvim, nvim_top) = support::nvim::run_on(&before, keys, Some(screen));
    assert_eq!((engine.text, engine_top), (nvim.text, nvim_top));
}

/// Donde nvim deja el cursor a mitad de una línea partida (en la segunda
/// fila de un `##`, columna 80), el heading no tiene filas por dentro: el
/// cursor queda en la línea, en su columna. Y la ventana se mueve de a
/// líneas enteras, así que el cursor baja las filas que bajó la ventana;
/// nvim, al deshacer la fila de más que movió con `smoothscroll`, baja una
/// fila más (la 96 en vez de la 95).
#[rstest]
#[case(screen(10, 0, 1), (3, 0), "<C-d><C-d>", (11, 0), 8)]
#[case(screen(10, 0, 21), (24, 0), "3<C-d>", (26, 0), 22)]
#[case(screen(10, 0, 1), (1, 0), "<C-f>", (6, 0), 6)]
#[case(screen(10, 0, 88), (92, 0), "<C-d>", (95, 0), 91)]
fn page_scrolls_by_whole_lines(
    #[case] screen: Screen,
    #[case] cursor: (usize, usize),
    #[case] keys: &str,
    #[case] after: (usize, usize),
    #[case] top: usize,
) {
    let (engine, first) = support::run_engine_on(&note(cursor), keys, Some(screen));
    let before = &engine.text[..engine.text.find('|').unwrap()];
    let line = before.matches('\n').count() + 1;
    let column = before.len() - before.rfind('\n').map_or(0, |i| i + 1);
    assert_eq!(((line, column), first + 1), (after, top));
}
