use pretty_assertions::assert_eq;
use proptest::prelude::*;

use super::*;

/// Cada carácter es un grafema: los de `wide` miden 2 y los demás 1.
fn widths(text: &str) -> Vec<f32> {
    text.chars()
        .map(|c| {
            if "日本語漢字".contains(c) {
                2.0
            } else {
                1.0
            }
        })
        .collect()
}

/// `text` recortado como lo pinta la vista, con elipsis de ancho 1.
fn shown_around(text: &str, focus: Range<usize>, budget: f32) -> String {
    let chars: Vec<char> = text.chars().collect();
    let fit = around(&widths(text), focus, budget, 1.0);
    let kept: String = chars[fit.kept].iter().collect();
    let before = if fit.ellipsis_before { "…" } else { "" };
    let after = if fit.ellipsis_after { "…" } else { "" };
    format!("{before}{kept}{after}")
}

fn shown_middle(text: &str, budget: f32) -> String {
    let chars: Vec<char> = text.chars().collect();
    let cut = middle(&widths(text), budget, 1.0, name_graphemes(text));
    let head: String = chars[..cut.head].iter().collect();
    let tail: String = chars[cut.tail..].iter().collect();
    let ellipsis = if cut.is_cut() { "…" } else { "" };
    format!("{head}{ellipsis}{tail}")
}

fn assert_close(found: f32, expected: f32) {
    assert!((found - expected).abs() < 1e-4, "{found} != {expected}");
}

fn width_of(shown: &str) -> f32 {
    widths(shown).iter().sum()
}

#[test]
fn lo_que_cabe_va_entero() {
    assert_eq!(shown_around("una idea", 4..8, 8.0), "una idea");
    assert_eq!(shown_around("", 0..0, 5.0), "");
}

#[test]
fn la_coincidencia_queda_centrada_con_elipsis_a_los_dos_lados() {
    let text = "aaaaaaaaaa idea bbbbbbbbbb";
    assert_eq!(shown_around(text, 11..15, 10.0), "…a idea b…");
    assert_close(width_of(&shown_around(text, 11..15, 10.0)), 10.0);
}

#[test]
fn cerca_de_un_borde_lo_que_sobra_va_al_otro_lado() {
    assert_eq!(
        shown_around("idea bbbbbbbbbbbbbbbbbbbb", 0..4, 10.0),
        "idea bbbb…"
    );
    assert_eq!(
        shown_around("aaaaaaaaaaaaaaaaaaaa idea", 21..25, 10.0),
        "…aaaa idea"
    );
}

#[test]
fn sin_foco_se_recorta_al_final() {
    assert_eq!(shown_around("abcdefghij", 0..0, 5.0), "abcd…");
}

#[test]
fn una_coincidencia_mas_ancha_que_la_fila_deja_su_principio() {
    assert_eq!(shown_around("xx abcdefghij yy", 3..13, 6.0), "…abcd…");
}

#[test]
fn los_grafemas_anchos_cuentan_por_lo_que_miden() {
    let text = "日本語漢字 idea 日本語漢字";
    let shown = shown_around(text, 6..10, 10.0);
    assert_eq!(shown, "…字 idea …");
    assert!(width_of(&shown) <= 10.0);
}

#[test]
fn sin_espacio_queda_una_sola_elipsis() {
    assert_eq!(shown_around("abcdef", 2..4, 0.5), "…");
    assert_eq!(shown_middle("abcdef", 0.5), "…");
}

#[test]
fn la_ruta_se_recorta_por_el_medio() {
    assert_eq!(
        shown_middle("notas/capitulo.md:3", 25.0),
        "notas/capitulo.md:3"
    );
    assert_eq!(
        shown_middle("personajes-largo-vin.md:3", 11.0),
        "perso….md:3"
    );
    assert_eq!(
        shown_middle("personajes-largo-vin.md:3", 12.0),
        "perso…n.md:3"
    );
    assert_eq!(
        shown_middle("personajes/largo/vin.md:3", 11.0),
        "p…/vin.md:3"
    );
    assert_eq!(
        shown_middle("personajes/largo/vin.md:3", 12.0),
        "p…o/vin.md:3"
    );
}

#[test]
fn el_nombre_de_la_ruta_queda_entero_si_cabe() {
    assert_eq!(
        shown_middle("personajes/equipo/historias/Vin con nombre largo.md", 34.0),
        "perso…rias/Vin con nombre largo.md"
    );
    assert_eq!(
        shown_middle("personajes/equipo/historias/Vin con nombre largo.md", 40.0),
        "personaj…storias/Vin con nombre largo.md"
    );
}

#[test]
fn un_nombre_que_no_cabe_se_recorta_por_el_medio() {
    assert_eq!(
        shown_middle("a/un nombre de nota larguísimo.md", 12.0),
        "a/un …imo.md"
    );
}

#[test]
fn la_ruta_con_grafemas_anchos_no_se_pasa() {
    let shown = shown_middle("日本語漢字/日本語漢字.md:1", 9.0);
    assert!(width_of(&shown) <= 9.0, "{shown}");
    assert!(shown.ends_with(":1"), "{shown}");
}

#[test]
fn si_caben_los_dos_cada_uno_se_queda_con_lo_suyo() {
    assert_eq!(split(100.0, 40.0, 20.0, 8.0, 0.4), (72.0, 20.0));
}

#[test]
fn un_texto_largo_le_deja_al_detalle_su_parte() {
    assert_eq!(split(108.0, 500.0, 60.0, 8.0, 0.4), (60.0, 40.0));
    assert_eq!(split(108.0, 500.0, 30.0, 8.0, 0.4), (70.0, 30.0));
}

#[test]
fn un_texto_corto_le_deja_al_detalle_lo_que_sobra() {
    assert_eq!(split(108.0, 20.0, 500.0, 8.0, 0.4), (20.0, 80.0));
}

#[test]
fn sin_detalle_el_texto_usa_toda_la_fila() {
    assert_eq!(split(100.0, 500.0, 0.0, 8.0, 0.4), (100.0, 0.0));
    assert_eq!(split(4.0, 500.0, 30.0, 8.0, 0.4), (0.0, 0.0));
}

#[test]
fn la_lista_mide_filas_enteras_mas_el_borde() {
    assert_close(list_height(0, 12, 33.0, 1.0), 34.0);
    assert_close(list_height(3, 12, 33.0, 1.0), 100.0);
    assert_close(list_height(40, 12, 33.0, 1.0), 397.0);
    assert_close(list_height(100_000, 12, 33.0, 1.0), 397.0);
    assert_close(list_height(5, 0, 33.0, 0.0), 33.0);
}

#[test]
fn los_resaltados_siguen_al_recorte() {
    let highlights = [0..2, 5..9, 12..14, 20..22];
    assert_eq!(cut_highlights(&highlights, 6..13, "…".len()), [3..6, 9..10]);
    assert_eq!(cut_highlights(&highlights, 0..22, 0), highlights);
    assert_eq!(
        cut_highlights(&highlights, 2..5, 0),
        Vec::<Range<usize>>::new()
    );
}

#[test]
fn los_resaltados_de_una_ruta_recortada_por_el_medio() {
    // "notas/largas/vin.md" → "notas…vin.md"
    let highlights = [0..2, 4..8, 10..12, 13..16];
    assert_eq!(
        middle_highlights(&highlights, 19, 5, 13, "…".len()),
        [0..2, 4..5, 8..11]
    );
}

#[test]
fn cada_grafema_mide_lo_que_avanzan_sus_glifos() {
    // "ab" + "e\u{301}" (un grafema de dos glifos) + "日": 4 grafemas.
    let starts = [0, 1, 2, 5, 8];
    let glyphs = [(0, 0.0), (1, 7.0), (2, 14.0), (3, 14.0), (5, 21.0)];
    let found = grapheme_widths(&starts, &glyphs, 35.0);
    assert_eq!(found.len(), 4);
    for (found, expected) in found.into_iter().zip([7.0, 7.0, 7.0, 14.0]) {
        assert_close(found, expected);
    }
    assert_eq!(grapheme_widths(&[0], &[], 0.0), Vec::<f32>::new());
}

#[test]
fn una_fila_es_una_sola_linea() {
    assert_eq!(one_line("a\nb\tc\r"), "a b c ");
    assert_eq!(one_line("niño").len(), "niño".len());
}

fn graphemes() -> impl Strategy<Value = Vec<f32>> {
    prop::collection::vec(prop::sample::select(vec![0.5_f32, 1.0, 2.0]), 0..60)
}

proptest! {
    #[test]
    fn around_nunca_se_pasa_y_muestra_la_coincidencia(
        widths in graphemes(),
        start in 0usize..60,
        span in 0usize..8,
        budget in 0.0_f32..40.0,
    ) {
        let len = widths.len();
        let focus = start.min(len)..(start + span).min(len);
        let fit = around(&widths, focus.clone(), budget, 1.0);
        let edges = f32::from(u8::from(fit.ellipsis_before)) + f32::from(u8::from(fit.ellipsis_after));
        let used: f32 = widths[fit.kept.clone()].iter().sum::<f32>() + edges;
        prop_assert!(used <= budget || fit.kept.is_empty());
        prop_assert!(fit.kept.end <= len);
        let focus_width: f32 = widths[focus.clone()].iter().sum();
        if focus_width + 2.0 <= budget {
            prop_assert!(fit.kept.start <= focus.start && focus.end <= fit.kept.end);
        }
        if widths.iter().sum::<f32>() <= budget {
            prop_assert_eq!(fit.kept, 0..len);
        }
    }

    #[test]
    fn middle_nunca_se_pasa(widths in graphemes(), budget in 0.0_f32..40.0, keep in 0usize..70) {
        let cut = middle(&widths, budget, 1.0, keep);
        let ellipsis = if cut.is_cut() { 1.0 } else { 0.0 };
        let used: f32 =
            widths[..cut.head].iter().sum::<f32>() + widths[cut.tail..].iter().sum::<f32>() + ellipsis;
        prop_assert!(cut.head <= cut.tail && cut.tail <= widths.len());
        prop_assert!(used <= budget || (cut.head == 0 && cut.tail == widths.len()));
    }
}
