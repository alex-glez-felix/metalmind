//! Una pizarra sintética y determinista: 60 % nodos (cajas de cada forma,
//! tarjetas de nota y algún grupo) y 40 % flechas entre vecinos, como en 09 §2.2.

use std::fmt::Write as _;

const COLUMN: i64 = 280;
const ROW: i64 = 160;
const WIDTH: i64 = 220;
const HEIGHT: i64 = 90;
const GROUP_EVERY: usize = 50;

/// El `.canvas` de una pizarra de `elements` elementos.
#[must_use]
pub fn board(elements: usize) -> String {
    let nodes = elements * 3 / 5;
    let columns = nodes.isqrt().max(1);
    let mut out = String::from("{\n\t\"nodes\":[\n");
    for index in 0..nodes {
        let separator = if index + 1 == nodes { "" } else { "," };
        let _ = writeln!(out, "\t\t{}{separator}", node(index, columns));
    }
    out.push_str("\t],\n\t\"edges\":[\n");
    let edges = (elements - nodes).min(nodes.saturating_sub(1));
    for index in 0..edges {
        let separator = if index + 1 == edges { "" } else { "," };
        let _ = writeln!(out, "\t\t{}{separator}", edge(index, nodes, columns));
    }
    out.push_str("\t]\n}\n");
    out
}

fn node(index: usize, columns: usize) -> String {
    let (column, row) = (index % columns, index / columns);
    let x = i64::try_from(column).unwrap_or(0) * COLUMN;
    let y = i64::try_from(row).unwrap_or(0) * ROW;
    let place = format!(r#""x":{x},"y":{y},"width":{WIDTH},"height":{HEIGHT}"#);
    let id = format!("n{index}");
    if index.is_multiple_of(GROUP_EVERY) {
        return format!(r#"{{"id":"{id}","type":"group",{place},"label":"Grupo {index}"}}"#);
    }
    match index % 7 {
        0 => format!(r#"{{"id":"{id}","type":"file",{place},"file":"notas/nota-{index}.md"}}"#),
        1 => format!(
            r#"{{"id":"{id}","type":"text",{place},"shape":"ellipse","text":"Elipse {index}"}}"#
        ),
        2 => format!(
            r#"{{"id":"{id}","type":"text",{place},"shape":"diamond","text":"¿Rombo {index}?"}}"#
        ),
        3 => format!(
            r#"{{"id":"{id}","type":"text",{place},"text":"Caja con color {index}","color":"4"}}"#
        ),
        _ => format!(
            r#"{{"id":"{id}","type":"text",{place},"text":"Una caja con más de una palabra adentro, la {index}"}}"#
        ),
    }
}

/// A su vecino de la derecha o, en la última columna, al de abajo.
fn edge(index: usize, nodes: usize, columns: usize) -> String {
    let to = if (index + 1).is_multiple_of(columns) && index + columns < nodes {
        index + columns
    } else {
        index + 1
    };
    let route = match index % 3 {
        0 => "",
        1 => r#","route":"straight""#,
        _ => r#","route":"elbow""#,
    };
    format!(r#"{{"id":"e{index}","fromNode":"n{index}","toNode":"n{to}"{route}}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn es_determinista_y_tiene_una_linea_por_elemento() {
        let text = board(2_000);
        assert_eq!(text, board(2_000));
        assert_eq!(text.lines().count(), 2_000 + 6);
    }

    #[test]
    fn una_pizarra_muy_chica_no_se_rompe() {
        for elements in 0..4 {
            assert!(board(elements).ends_with("}\n"));
        }
    }
}
