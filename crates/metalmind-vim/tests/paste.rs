#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Lo pegado fuera de Insert, como el `ctrl-shift-v` de la app. No va contra
//! el oráculo: el pegado de nvim en Normal es otro (pone el texto después
//! del cursor, como `p`).

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use pretty_assertions::assert_eq;
use rstest::rstest;

#[rstest]
#[case::en_el_cursor("a|bc", "<PasteStart>xy<PasteEnd>", "ax|ybc")]
#[case::un_solo_paso_de_undo("a|bc", "<PasteStart>xy<PasteEnd>u", "a|bc")]
#[case::cancela_el_count("a|bcdef", "3<PasteStart>x<PasteEnd>lx", "ax|cdef")]
#[case::cancela_el_operador("a|bc", "d<PasteStart>x<PasteEnd>l", "ax|bc")]
#[case::sale_de_visual("|abc", "vl<PasteStart>x<PasteEnd>", "a|xbc")]
#[case::varias_lineas("a|b", "<PasteStart>x\ny<PasteEnd>", "ax\n|yb")]
fn pasting_outside_insert(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let state = support::run_engine(before, keys);
    assert_eq!(state.text, after);
    assert_eq!(state.mode, "n");
}

/// Lo pegado mueve las marks `A`–`Z` y la jumplist, como cualquier edición.
#[test]
fn pasting_moves_the_file_marks() {
    let state = support::run_engine("a\n|b", "mAgg<PasteStart>x\n<PasteEnd>'A");
    assert_eq!(state.text, "x\na\n|b");
}
