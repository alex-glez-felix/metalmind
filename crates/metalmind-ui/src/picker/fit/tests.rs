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

fn starts(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(start, _)| start)
        .chain([text.len()])
        .collect()
}

/// `text` recortado como lo pinta la vista, con elipsis de ancho 1.
fn shown(text: &str, kept: &Kept) -> String {
    render(text, &starts(text), kept, &[]).0
}

fn shown_around(text: &str, focus: Range<usize>, budget: f32) -> String {
    shown(text, &around(&widths(text), focus, budget, 1.0))
}

fn shown_detail(text: &str, budget: f32) -> String {
    shown(
        text,
        &path(&widths(text), &slashes(text), None, budget, 1.0),
    )
}

/// Una ruta con `found` (lo que coincide) resaltado, como `[found]`.
fn shown_path(text: &str, found: &str, budget: f32) -> String {
    let at = text.find(found).unwrap();
    let focus = text[..at].chars().count()..text[..at + found.len()].chars().count();
    let kept = path(&widths(text), &slashes(text), Some(focus), budget, 1.0);
    let (mut shown, highlights) = render(
        text,
        &starts(text),
        &kept,
        std::slice::from_ref(&(at..at + found.len())),
    );
    for range in highlights.iter().rev() {
        shown.insert(range.end, ']');
        shown.insert(range.start, '[');
    }
    shown
}

/// Lo que mide `kept` pintado: sus grafemas y una elipsis por hueco.
fn used(widths: &[f32], kept: &Kept) -> f32 {
    let text: String = "x".repeat(kept.len);
    let gaps = render(&text, &starts(&text), kept, &[])
        .0
        .matches(ELLIPSIS)
        .count();
    let shown: f32 = kept
        .ranges
        .iter()
        .map(|range| widths[range.clone()].iter().sum::<f32>())
        .sum();
    shown + f32::from(u8::try_from(gaps).unwrap())
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
    assert_eq!(shown_detail("abcdef", 0.5), "…");
}

#[test]
fn la_ruta_se_recorta_por_el_medio() {
    assert_eq!(
        shown_detail("notas/capitulo.md:3", 25.0),
        "notas/capitulo.md:3"
    );
    assert_eq!(
        shown_detail("personajes-largo-vin.md:3", 11.0),
        "perso….md:3"
    );
    assert_eq!(
        shown_detail("personajes-largo-vin.md:3", 12.0),
        "perso…n.md:3"
    );
    assert_eq!(
        shown_detail("personajes/largo/vin.md:3", 11.0),
        "…o/vin.md:3"
    );
    assert_eq!(
        shown_detail("personajes/largo/vin.md:3", 12.0),
        "…go/vin.md:3"
    );
}

#[test]
fn el_nombre_de_la_ruta_queda_entero_si_cabe() {
    assert_eq!(
        shown_detail("personajes/equipo/historias/Vin con nombre largo.md", 34.0),
        "…historias/Vin con nombre largo.md"
    );
    assert_eq!(
        shown_detail("personajes/equipo/historias/Vin con nombre largo.md", 40.0),
        "…quipo/historias/Vin con nombre largo.md"
    );
}

#[test]
fn un_nombre_que_no_cabe_se_recorta_por_el_medio() {
    assert_eq!(
        shown_detail("a/un nombre de nota larguísimo.md", 12.0),
        "…/un n…mo.md"
    );
}

#[test]
fn la_ruta_con_grafemas_anchos_no_se_pasa() {
    let shown = shown_detail("日本語漢字/日本語漢字.md:1", 9.0);
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
    let text = "abcdefghijklmnopqrstuv";
    let kept = Kept {
        ranges: std::iter::once(6..13).collect(),
        len: 22,
    };
    let (shown, highlights) = render(text, &starts(text), &kept, &[0..2, 5..9, 12..14, 20..22]);
    assert_eq!(shown, "…ghijklm…");
    assert_eq!(highlights, [3..6, 9..10]);
}

#[test]
fn los_resaltados_de_una_ruta_recortada_por_el_medio() {
    let text = "notas/largas/vin.md";
    let kept = Kept {
        ranges: vec![0..5, 13..19],
        len: 19,
    };
    let (shown, highlights) = render(text, &starts(text), &kept, &[0..2, 4..8, 10..12, 13..16]);
    assert_eq!(shown, "notas…vin.md");
    assert_eq!(highlights, [0..2, 4..5, 8..11]);
}

#[test]
fn una_ruta_muestra_lo_que_coincide_y_el_nombre() {
    let text = "personajes/equipo de Kelsier/historias largas/Vin.md";
    assert_eq!(
        shown_path(text, "historias", 30.0),
        "…/[historias] largas/Vin.md"
    );
}

#[test]
fn una_ruta_con_la_coincidencia_en_el_nombre_se_recorta_por_el_medio() {
    let text = "personajes/equipo de Kelsier/historias largas/Vin.md";
    assert_eq!(shown_path(text, "Vin", 20.0), "…orias largas/[Vin].md");
}

const FOLDER: &str = "personajes/equipo de Kelsier/historias largas de la Última Imperio";

#[test]
fn dos_rutas_que_solo_cambian_en_el_nombre_se_distinguen() {
    let vin = format!("{FOLDER}/Vin con un nombre de nota bastante largo para probar.md:5");
    let sazed = format!("{FOLDER}/Sazed con un nombre de nota bastante largo para probar.md:5");
    assert_eq!(shown_detail(&vin, 26.0), "…/Vin con un … probar.md:5");
    assert_eq!(shown_detail(&sazed, 26.0), "…/Sazed con u… probar.md:5");
}

#[test]
fn con_la_coincidencia_en_la_carpeta_se_ve_tambien_el_nombre() {
    let kelsier = format!("{FOLDER}/Kelsier con un nombre de nota bastante largo para probar.md");
    assert_eq!(
        shown_path(&kelsier, "historias", 40.0),
        "…/[historias]…/Kelsier con u…ara probar.md"
    );
}

#[test]
fn todas_las_filas_de_una_ruta_empiezan_igual() {
    for name in ["Vin", "Elend", "Sazed", "Kelsier"] {
        let note = format!("{FOLDER}/{name} con un nombre de nota bastante largo para probar.md");
        let shown = shown_path(&note, "historias", 50.0);
        assert!(shown.starts_with("…/[historias]"), "{shown}");
        assert!(shown.contains(&format!("/{name} ")), "{shown}");
    }
}

#[test]
fn si_no_caben_la_coincidencia_y_el_nombre_gana_la_coincidencia() {
    let text = "personajes/equipo de Kelsier/historias/un nombre larguísimo.md";
    assert_eq!(
        shown_path(text, "historias", 20.0),
        "…/[historias]/un …o.md"
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
fn de_derecha_a_izquierda_cada_glifo_es_de_su_grafema() {
    // Tres letras hebreas de dos bytes: los glifos van al revés de los bytes.
    let starts = [0, 2, 4, 6];
    let glyphs = [(4, 0.0), (2, 5.0), (0, 12.0)];
    let found = grapheme_widths(&starts, &glyphs, 20.0);
    for (found, expected) in found.into_iter().zip([8.0, 7.0, 5.0]) {
        assert_close(found, expected);
    }
}

#[test]
fn una_ligadura_reparte_su_ancho_entre_los_grafemas_que_junta() {
    // "fi" en un solo glifo de 8 y una "x" de 6.
    let starts = [0, 1, 2, 3];
    let glyphs = [(0, 0.0), (2, 8.0)];
    let found = grapheme_widths(&starts, &glyphs, 14.0);
    for (found, expected) in found.into_iter().zip([4.0, 4.0, 6.0]) {
        assert_close(found, expected);
    }
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
        let kept = around(&widths, focus.clone(), budget, 1.0);
        prop_assert!(kept.ranges.len() <= 1);
        prop_assert!(used(&widths, &kept) <= budget || kept.ranges.is_empty());
        let focus_width: f32 = widths[focus.clone()].iter().sum();
        if focus_width + 2.0 <= budget {
            prop_assert!(kept.shows(&focus) || focus.is_empty());
        }
        if widths.iter().sum::<f32>() <= budget {
            prop_assert_eq!(kept.ranges, if len == 0 { vec![] } else { std::iter::once(0..len).collect() });
        }
    }

    #[test]
    fn middle_nunca_se_pasa(widths in graphemes(), budget in 0.0_f32..40.0) {
        let kept = middle(&widths, budget, 1.0);
        prop_assert!(kept.ranges.len() <= 2);
        prop_assert!(used(&widths, &kept) <= budget || kept.ranges.is_empty());
    }

    #[test]
    fn path_nunca_se_pasa_y_guarda_lo_que_importa(
        widths in graphemes(),
        slashes in prop::collection::btree_set(0usize..60, 0..5),
        focus in prop::option::of((0usize..60, 1usize..8)),
        budget in 0.0_f32..40.0,
    ) {
        let len = widths.len();
        let slashes: Vec<usize> = slashes.into_iter().filter(|&slash| slash < len).collect();
        let focus = focus.map(|(start, span)| start.min(len)..(start + span).min(len));
        let kept = path(&widths, &slashes, focus.clone(), budget, 1.0);
        prop_assert!(used(&widths, &kept) <= budget || kept.ranges.is_empty());
        prop_assert!(kept.ranges.iter().all(|range| range.end <= len));
        if let Some(focus) = focus.filter(|focus| !focus.is_empty()) {
            let focus_width: f32 = widths[focus.clone()].iter().sum();
            if focus_width + 2.0 <= budget {
                prop_assert!(kept.shows(&focus));
            }
        } else if let Some(&name_slash) = slashes.last() {
            let name: f32 = widths[name_slash..].iter().sum();
            if name + 1.0 <= budget {
                prop_assert!(kept.shows(&(name_slash..len)));
            }
        }
    }
}
