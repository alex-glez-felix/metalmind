#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Scroll y `H`, `M`, `L` en una ventana de tamaño fijo. Cada caso corre en
//! el motor y en `nvim --clean` con las mismas filas y `scrolloff`, y
//! compara el cursor y la primera línea visible (`line('w0')`). Las líneas
//! cuentan desde 1, como en vim; la columna, en bytes desde 0. nota no parte
//! las líneas largas, así que todas caben en las 80 columnas de nvim.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};
use support::Screen;

/// `n` líneas numeradas de largos distintos, para que se note la columna.
fn numbered(n: usize) -> String {
    (1..=n)
        .map(|i| format!("{i:03}{}", "-".repeat(i * 7 % 13)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Como [`numbered`], con sangría para `z<CR>`, `z.` y `z-`.
fn indented(n: usize) -> String {
    (1..=n)
        .map(|i| format!("    {i:03}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Copy, Debug)]
enum Doc {
    Numbered(usize),
    Indented(usize),
}

/// El texto con el cursor marcado en `line` (desde 1) y `column`.
fn marked(doc: Doc, (line, column): (usize, usize)) -> String {
    let text = match doc {
        Doc::Numbered(n) => numbered(n),
        Doc::Indented(n) => indented(n),
    };
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    lines[line - 1].insert(column, '|');
    lines.join("\n")
}

const fn screen(rows: usize, scrolloff: usize, top: usize) -> Screen {
    Screen {
        rows,
        scrolloff,
        top: top - 1,
    }
}

/// `(texto, ventana, cursor, teclas, cursor después, primera línea después)`.
#[template]
#[rstest]
// ctrl-d y ctrl-u: media pantalla, y el count queda para las siguientes
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "<C-d>", (6, 0), 6)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (3, 2), "<C-d><C-d>", (13, 2), 11)]
#[case(Doc::Numbered(100), screen(10, 0, 41), (45, 0), "<C-u>", (40, 0), 36)]
#[case(Doc::Numbered(100), screen(10, 0, 3), (5, 0), "<C-u>", (1, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "3<C-d>", (4, 0), 4)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "3<C-d><C-d>", (7, 0), 7)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "3<C-d>2<C-d><C-u>", (4, 0), 4)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "50<C-d>", (11, 0), 11)]
#[case(Doc::Numbered(100), screen(10, 0, 51), (55, 0), "3<C-u><C-u>", (49, 0), 45)]
// ctrl-d sin pasar del final, y en los bordes
#[case(Doc::Numbered(100), screen(10, 0, 88), (92, 0), "<C-d>", (97, 0), 91)]
#[case(Doc::Numbered(100), screen(10, 0, 91), (95, 0), "<C-d>", (100, 0), 91)]
#[case(Doc::Numbered(100), screen(10, 0, 91), (100, 0), "<C-d>", (100, 0), 91)]
#[case(Doc::Numbered(100), screen(10, 0, 91), (100, 0), "<C-d>x", (100, 0), 91)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "<C-u>", (1, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (4, 0), "<C-u>", (1, 0), 1)]
// ctrl-d olvida la columna buscada; `$` llega al final de cada línea
#[case(Doc::Numbered(100), screen(10, 0, 1), (9, 13), "<C-d>j", (15, 3), 6)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (9, 13), "jj<C-d>", (16, 10), 7)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (9, 13), "<C-d>jj", (16, 10), 7)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (2, 0), "$<C-d>", (7, 12), 6)]
// ctrl-f y ctrl-b: pantalla completa menos dos líneas
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "<C-f>", (9, 0), 9)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "2<C-f>", (17, 0), 17)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "<C-f><C-b>", (10, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 41), (45, 0), "<C-b>", (42, 0), 33)]
#[case(Doc::Numbered(100), screen(10, 0, 89), (95, 0), "<C-f>", (97, 0), 97)]
#[case(Doc::Numbered(100), screen(10, 0, 91), (95, 0), "<C-f>", (100, 0), 100)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 0), "<C-b>", (5, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "50<C-f>", (100, 0), 100)]
#[case(Doc::Numbered(100), screen(4, 0, 1), (1, 0), "<C-f>", (4, 0), 4)]
#[case(Doc::Numbered(100), screen(3, 0, 1), (1, 0), "<C-f>", (4, 0), 4)]
// ctrl-e y ctrl-y: una línea, el cursor solo si se sale
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 0), "<C-e>", (5, 0), 2)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 3), "<C-e>", (2, 3), 2)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (9, 13), "10<C-e>", (11, 13), 11)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (9, 13), "10<C-e>j", (12, 8), 11)]
#[case(Doc::Numbered(100), screen(10, 0, 91), (95, 0), "200<C-e>", (100, 0), 100)]
#[case(Doc::Numbered(100), screen(10, 0, 11), (15, 0), "<C-y>", (15, 0), 10)]
#[case(Doc::Numbered(100), screen(10, 0, 11), (20, 0), "<C-y>", (19, 0), 10)]
#[case(Doc::Numbered(100), screen(10, 0, 11), (20, 0), "3<C-y>", (17, 0), 8)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 0), "<C-y>", (5, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 0), "<C-y>x", (5, 0), 1)]
// zt, zz, zb y sus variantes al primer no blanco
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 2), "zt", (5, 2), 5)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 2), "zz", (5, 2), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "zz", (25, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "zb", (25, 2), 16)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 2), "zb", (5, 2), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 91), (100, 2), "zz", (100, 2), 96)]
#[case(Doc::Numbered(100), screen(10, 0, 91), (100, 2), "zt", (100, 2), 100)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 2), "50zt", (50, 2), 50)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 2), "50zb''", (5, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (5, 2), "500zz", (100, 2), 96)]
#[case(Doc::Indented(100), screen(10, 0, 21), (25, 0), "z<CR>", (25, 4), 25)]
#[case(Doc::Indented(100), screen(10, 0, 21), (25, 0), "z.", (25, 4), 21)]
#[case(Doc::Indented(100), screen(10, 0, 21), (25, 0), "z-", (25, 4), 16)]
#[case(Doc::Indented(100), screen(10, 0, 21), (25, 0), "30z<CR>", (30, 4), 30)]
// H, M y L, con count y como saltos
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "H", (21, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "M", (25, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "L", (30, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "3H", (23, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "3L", (28, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "30L", (21, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "30H", (30, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "L''", (25, 0), 21)]
#[case(Doc::Numbered(100), screen(9, 0, 21), (25, 2), "M", (25, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (22, 13), "Lj", (31, 11), 22)]
// H y L con scrolloff, que no aplica con un operador
#[case(Doc::Numbered(100), screen(10, 3, 21), (25, 2), "H", (24, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 3, 21), (25, 2), "L", (27, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 3, 21), (25, 2), "2L", (27, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (5, 2), "H", (1, 2), 1)]
#[case(Doc::Numbered(100), screen(10, 3, 91), (95, 2), "L", (100, 2), 91)]
#[case(Doc::Numbered(100), screen(10, 3, 21), (25, 2), "dL", (25, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 3, 21), (25, 2), "dH", (21, 2), 18)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "d3L", (25, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "yM", (25, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "dL.", (25, 2), 21)]
// con un operador pendiente, el scroll lo cancela
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "d<C-d>", (25, 2), 21)]
#[case(Doc::Numbered(100), screen(10, 0, 21), (25, 2), "dztx", (25, 2), 21)]
// scrolloff al mover el cursor
#[case(Doc::Numbered(100), screen(10, 3, 1), (6, 0), "j", (7, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (6, 0), "jjj", (9, 0), 3)]
#[case(Doc::Numbered(100), screen(10, 3, 11), (14, 0), "k", (13, 0), 10)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (4, 0), "k", (3, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (1, 0), "G", (100, 0), 91)]
#[case(Doc::Numbered(100), screen(10, 3, 91), (99, 0), "gg", (1, 0), 1)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (1, 0), "50G", (50, 0), 46)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (1, 0), "50G", (50, 0), 46)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (5, 0), "8j", (13, 0), 7)]
#[case(Doc::Numbered(100), screen(10, 0, 1), (10, 0), "j", (11, 0), 2)]
#[case(Doc::Numbered(100), screen(10, 99, 1), (1, 0), "20j", (21, 0), 17)]
#[case(Doc::Numbered(100), screen(10, 99, 1), (1, 0), "20jk", (20, 0), 16)]
// scroll con scrolloff
#[case(Doc::Numbered(100), screen(10, 3, 1), (1, 0), "<C-d>", (9, 0), 6)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (4, 0), "<C-e>", (5, 0), 2)]
#[case(Doc::Numbered(100), screen(10, 3, 11), (17, 0), "<C-y>", (16, 0), 10)]
#[case(Doc::Numbered(100), screen(10, 3, 1), (1, 0), "<C-f>", (12, 0), 9)]
#[case(Doc::Numbered(100), screen(10, 3, 41), (45, 0), "<C-b>", (39, 0), 33)]
#[case(Doc::Numbered(100), screen(10, 3, 91), (97, 0), "<C-e>", (97, 0), 92)]
#[case(Doc::Numbered(100), screen(10, 3, 91), (95, 0), "<C-d>", (100, 0), 91)]
#[case(Doc::Numbered(100), screen(10, 3, 21), (25, 0), "zt", (25, 0), 22)]
#[case(Doc::Numbered(100), screen(10, 3, 21), (25, 0), "zb", (25, 0), 19)]
#[case(Doc::Numbered(100), screen(10, 99, 1), (1, 0), "<C-d>", (10, 0), 6)]
// un archivo más corto que la ventana
#[case(Doc::Numbered(5), screen(10, 0, 1), (2, 0), "<C-d>", (5, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (2, 0), "<C-u>", (1, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (2, 0), "<C-f>", (5, 0), 5)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (2, 0), "<C-e>", (2, 0), 2)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (2, 0), "<C-e>k", (1, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (2, 0), "L", (5, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (2, 0), "M", (3, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (4, 0), "H", (1, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (4, 0), "zt", (4, 0), 4)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (4, 0), "zb", (4, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 0, 1), (4, 0), "zz", (4, 0), 1)]
#[case(Doc::Numbered(5), screen(10, 3, 1), (4, 0), "<C-e>", (5, 0), 2)]
#[case(Doc::Numbered(1), screen(10, 3, 1), (1, 0), "<C-d><C-f><C-e>zbL", (1, 0), 1)]
fn cases(
    #[case] doc: Doc,
    #[case] screen: Screen,
    #[case] cursor: (usize, usize),
    #[case] keys: &str,
    #[case] after: (usize, usize),
    #[case] top: usize,
) {
}

/// Línea (desde 1) y columna del cursor en el texto marcado.
fn cursor_of(marked: &str) -> (usize, usize) {
    let before = &marked[..marked.find('|').unwrap()];
    let line = before.matches('\n').count() + 1;
    let column = before.len() - before.rfind('\n').map_or(0, |i| i + 1);
    (line, column)
}

#[apply(cases)]
fn engine(
    #[case] doc: Doc,
    #[case] screen: Screen,
    #[case] cursor: (usize, usize),
    #[case] keys: &str,
    #[case] after: (usize, usize),
    #[case] top: usize,
) {
    let (state, first) = support::run_engine_on(&marked(doc, cursor), keys, Some(screen));
    assert_eq!((cursor_of(&state.text), first + 1), (after, top));
}

#[apply(cases)]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_oracle(
    #[case] doc: Doc,
    #[case] screen: Screen,
    #[case] cursor: (usize, usize),
    #[case] keys: &str,
    #[case] after: (usize, usize),
    #[case] top: usize,
) {
    let before = marked(doc, cursor);
    let (nvim, first) = support::nvim::run_on(&before, keys, Some(screen));
    assert_eq!(
        (cursor_of(&nvim.text), first + 1),
        (after, top),
        "la tabla no coincide con nvim"
    );
    let (engine, _) = support::run_engine_on(&before, keys, Some(screen));
    assert_eq!(engine.text, nvim.text);
    assert_eq!(engine.mode, nvim.mode);
}

/// En Visual, `(antes, teclas)`: el motor y nvim terminan con la misma
/// selección y la misma primera línea.
#[rstest]
#[case((13, 0), "v<C-d>")]
#[case((13, 2), "v<C-d><C-d>d")]
#[case((15, 0), "V<C-u>")]
#[case((13, 0), "V<C-f>")]
#[case((15, 0), "v<C-b>")]
#[case((15, 2), "v<C-e>")]
#[case((13, 2), "v5<C-e>")]
#[case((15, 2), "vL")]
#[case((15, 2), "vHd")]
#[case((15, 2), "vM")]
#[case((15, 2), "vzt")]
#[case((15, 2), "v$<C-d>")]
#[case((15, 2), "vz<CR>")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn visual_matches_nvim(#[case] cursor: (usize, usize), #[case] keys: &str) {
    let before = marked(Doc::Numbered(100), cursor);
    let screen = screen(10, 2, 11);
    let (nvim, nvim_top) = support::nvim::run_on(&before, keys, Some(screen));
    let (engine, engine_top) = support::run_engine_on(&before, keys, Some(screen));
    assert_eq!((engine, engine_top), (nvim, nvim_top));
}
