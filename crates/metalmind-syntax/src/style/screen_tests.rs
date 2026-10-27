#![expect(
    clippy::float_cmp,
    reason = "las medidas son múltiplos exactos de la celda"
)]

use pretty_assertions::assert_eq;

use super::*;
use crate::{Styles, TextLines};

/// `text` en filas de `width` celdas, con cada carácter de una celda.
fn rows(text: &str, width: f32) -> Vec<&str> {
    let style = style_of(text);
    let line = ScreenLine::new(
        text,
        Advances::monospace(text, 1.0),
        Some(&style),
        Some(width),
    );
    (0..line.rows()).map(|row| &text[line.row(row)]).collect()
}

fn style_of(text: &str) -> LineStyle {
    let lines = TextLines::new(text);
    let mut styles = Styles::new(1);
    styles.ensure(0, &lines);
    styles.line(0).cloned().unwrap()
}

#[test]
fn parte_tras_el_espacio_y_lo_deja_colgando() {
    let cases: [(&str, f32, &[&str]); 8] = [
        ("hola mundo cruel", 10.0, &["hola mundo ", "cruel"]),
        ("aaaaaaaaaaaaaaa", 10.0, &["aaaaaaaaaa", "aaaaa"]),
        ("ab cdefghijklmnopq", 10.0, &["ab ", "cdefghijkl", "mnopq"]),
        ("aaaaaaaaaa   b", 10.0, &["aaaaaaaaaa   ", "b"]),
        ("dos  espacios", 5.0, &["dos  ", "espac", "ios"]),
        ("", 10.0, &[""]),
        ("corta", 10.0, &["corta"]),
        ("   ", 1.0, &["   "]),
    ];
    for (text, width, expected) in cases {
        assert_eq!(rows(text, width), expected, "{text:?} en {width}");
    }
}

#[test]
fn una_formula_no_se_parte_si_cabe() {
    assert_eq!(rows("ab $x + y$ cd", 8.0), ["ab ", "$x + y$ ", "cd"]);
    assert_eq!(
        rows("ab $x + y + z + w$", 6.0),
        ["ab $x ", "+ y + ", "z + w$"],
        "la que no cabe en una fila se parte como texto"
    );
}

#[test]
fn un_grafema_no_se_parte() {
    let text = "ae\u{301}e\u{301}e\u{301}";
    assert_eq!(rows(text, 2.0), ["ae\u{301}", "e\u{301}e\u{301}"]);
    assert_eq!(
        rows("日本語", 4.0),
        ["日本", "語"],
        "un carácter ancho mide dos"
    );
}

#[test]
fn sin_wrap_es_una_fila() {
    let text = "una línea muy larga que no se parte";
    let line = ScreenLine::new(text, Advances::monospace(text, 1.0), None, None);
    assert_eq!((line.rows(), line.row(0)), (1, 0..text.len()));
}

#[test]
fn cada_byte_en_su_fila_y_su_x() {
    let text = "hola mundo cruel";
    let line = ScreenLine::new(text, Advances::monospace(text, 2.0), None, Some(20.0));
    assert_eq!(line.row(1), 11..16);
    let rows: Vec<usize> = [0, 10, 11, 15, 16].map(|byte| line.row_of(byte)).to_vec();
    assert_eq!(rows, [0, 0, 1, 1, 1]);
    assert_eq!((line.x(11), line.x(13), line.end(13)), (0.0, 4.0, 6.0));
    assert_eq!(line.x(16), 10.0, "el fin de línea, tras el último");
    assert_eq!(line.byte_at(1, 5.0), 13);
    assert_eq!(
        line.byte_at(1, 99.0),
        15,
        "a la derecha, el último de la fila"
    );
    assert_eq!(
        line.byte_at(0, 99.0),
        10,
        "el espacio que cuelga es de la fila"
    );
    assert_eq!(line.byte_at(1, -3.0), 11);
}

#[test]
fn los_bytes_de_un_caracter_caen_en_el() {
    let advances = Advances::monospace("añb", 10.0);
    assert_eq!(
        [0, 1, 2, 3, 4].map(|byte| advances.x(byte)),
        [0.0, 10.0, 10.0, 20.0, 30.0]
    );
    assert_eq!([1, 2].map(|byte| advances.end(byte)), [20.0, 20.0]);
    assert_eq!(
        [0.0, 9.9, 10.0, 25.0, 30.0].map(|x| advances.byte_at(x)),
        [0, 0, 1, 3, 4]
    );
    assert_eq!([3, 4].map(|byte| advances.char_before(byte)), [1, 3]);
    assert_eq!(
        [0.0, 5.0, 10.0, 31.0].map(|x| advances.first_from(x)),
        [Some(0), Some(1), Some(1), None]
    );
    assert_eq!(
        [5.0, 10.0, 25.0, 30.0].map(|x| advances.last_until(x)),
        [None, Some(0), Some(1), Some(3)]
    );
}
