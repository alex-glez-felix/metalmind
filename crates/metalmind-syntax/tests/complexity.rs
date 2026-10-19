//! El scan corre en cada cambio de la nota y el estilo de las líneas en cada
//! tecla: lineales en el texto, también en lo patológico, y una tecla no
//! depende del largo de la nota. Ver `docs/performance.md`.

#[allow(
    dead_code,
    reason = "el benchmark `style` usa otra parte del mismo soporte"
)]
#[path = "support/styled.rs"]
mod styled;

use metalmind_perf::{
    assert_flat, assert_linear, backtick_runs, dollar_runs, hash_runs, huge_line, math_note, note,
    unclosed_link_titles, unclosed_wikilinks,
};
use metalmind_syntax::{Lines, Styles, scan};
use styled::{NoteLines, StyledNote};

#[test]
fn a_note_scans_in_linear_time() {
    assert_linear(1_000, note, |text| scan(&text));
}

#[test]
fn hash_runs_scan_in_linear_time() {
    assert_linear(2_000, hash_runs, |text| scan(&text));
}

#[test]
fn unclosed_wikilinks_scan_in_linear_time() {
    assert_linear(10_000, unclosed_wikilinks, |text| scan(&text));
}

#[test]
fn unclosed_link_titles_scan_in_linear_time() {
    assert_linear(5_000, unclosed_link_titles, |text| scan(&text));
}

#[test]
fn a_huge_line_scans_in_linear_time() {
    assert_linear(100_000, huge_line, |text| scan(&text));
}

#[test]
fn unclosed_dollars_scan_in_linear_time() {
    assert_linear(10_000, dollar_runs, |text| scan(&text));
}

#[test]
fn formulas_scan_in_linear_time() {
    assert_linear(5_000, math_note, |text| scan(&text));
}

/// Rachas de 1 a k: el texto mide ~k²/2, así que k crece con la raíz.
#[test]
fn backtick_runs_scan_in_linear_time() {
    assert_linear(
        100_000,
        |bytes| backtick_runs((2 * bytes).isqrt()),
        |text| scan(&text),
    );
}

fn style_all(text: &str) -> Styles {
    let lines = NoteLines::new(text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(usize::MAX, &lines);
    styles
}

#[test]
fn styling_a_note_is_linear() {
    assert_linear(1_000, note, |text| style_all(&text));
}

#[test]
fn styling_hash_runs_is_linear() {
    assert_linear(2_000, hash_runs, |text| style_all(&text));
}

#[test]
fn styling_unclosed_wikilinks_is_linear() {
    assert_linear(10_000, unclosed_wikilinks, |text| style_all(&text));
}

#[test]
fn styling_unclosed_link_titles_is_linear() {
    assert_linear(5_000, unclosed_link_titles, |text| style_all(&text));
}

#[test]
fn styling_a_huge_line_is_linear() {
    assert_linear(100_000, huge_line, |text| style_all(&text));
}

#[test]
fn styling_unclosed_dollars_is_linear() {
    assert_linear(10_000, dollar_runs, |text| style_all(&text));
}

#[test]
fn styling_formulas_is_linear() {
    assert_linear(2_000, math_note, |text| style_all(&text));
}

/// Bloques `$$` de varias líneas seguidos, sin una línea en blanco: tras
/// cada cierre el estilo sigue en el mismo párrafo.
#[test]
fn styling_blocks_in_one_paragraph_is_linear() {
    assert_linear(
        2_000,
        |blocks| "texto `a\n$$\nx\n$$ y\n".repeat(blocks),
        |text| style_all(&text),
    );
}

#[test]
fn styling_unpaired_emphasis_is_linear() {
    assert_linear(10_000, emphasis_runs, |text| style_all(&text));
}

/// Rachas de `*` y `_` sin pareja en un solo párrafo: cada cierre busca su
/// apertura hacia atrás, y sin el tope de cmark eso es cuadrático.
fn emphasis_runs(count: usize) -> String {
    (0..count)
        .map(|i| if i % 2 == 0 { "*a " } else { "_b_ **c " })
        .collect()
}

#[test]
fn a_key_costs_the_same_in_a_long_note() {
    assert_flat(
        1_000,
        |lines| StyledNote::new(&note(lines)),
        |mut note| {
            for _ in 0..8 {
                note.type_in_the_middle();
            }
            note
        },
    );
}

#[test]
fn opening_a_fence_paints_only_the_window() {
    assert_flat(
        1_000,
        |lines| StyledNote::new(&note(lines)),
        |mut note| {
            for _ in 0..8 {
                note.open_a_fence_at_the_top();
            }
            note
        },
    );
}
