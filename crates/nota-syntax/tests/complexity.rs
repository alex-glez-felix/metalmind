//! El scan corre en cada cambio de la nota: tiene que ser lineal en el texto,
//! también en lo patológico. Ver `docs/performance.md`.

use nota_perf::{
    assert_linear, backtick_runs, hash_runs, huge_line, note, unclosed_link_titles,
    unclosed_wikilinks,
};
use nota_syntax::scan;

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

/// Rachas de 1 a k: el texto mide ~k²/2, así que k crece con la raíz.
#[test]
fn backtick_runs_scan_in_linear_time() {
    assert_linear(
        100_000,
        |bytes| backtick_runs((2 * bytes).isqrt()),
        |text| scan(&text),
    );
}
