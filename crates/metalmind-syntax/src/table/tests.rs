use std::fmt::Write as _;

use pretty_assertions::assert_eq;
use proptest::prelude::*;

use super::{Align, CellPos, CellSpot, Step, Table};
use crate::TextLines;

/// Marca el cursor en los casos: `|` es de la tabla.
const CURSOR: char = '‸';

/// `(qué prueba, nota con el cursor)`.
const CASES: &[(&str, &str)] = &[
    (
        "desalineada",
        "| Criterio | Nota |\n|-|-|\n| Comunicación clara | 4‸ |\n| Código | 5 |",
    ),
    (
        "alineados declarados",
        "| a | b | c | d |\n|:-|:-:|-:|---|\n| izq‸uierda | centro | 7 | x |\n| i | c | 1234 | y |",
    ),
    (
        "ancho en columnas, no bytes",
        "| nombre | x |\n|---|---|\n| José‸ | 1 |\n| 東京 | 2 |\n| 🦀 | 3 |",
    ),
    (
        "pipes que no parten",
        "| qué | cómo |\n|---|---|\n| `a | b`‸ | código |\n| a \\| b | escapado |\n| [[nota|alias]] | wikilink |\n| $|x|$ | fórmula |",
    ),
    (
        "filas cortas y largas",
        "| a | b | c |\n|---|---|---|\n| 1‸ |\n| 1 | 2 | 3 | 4 | 5 |",
    ),
    ("sin pipes en los bordes", "a | b\n--|--\nuno‸ | dos"),
    ("cursor en la separadora", "| a | b |\n|-‸|:-|\n| x | y |"),
    ("cursor en un pipe", "| ab |‸ cd |\n|-|-|"),
    ("cursor tras el último pipe", "| ab | cd |  ‸\n|-|-|"),
    ("cursor en el relleno", "|   ‸   ab | cd |\n|-|-|"),
    ("celdas vacías", "|  | b |\n|-|-|\n|‸| |"),
    ("en una cita", "> | a | b |\n> |-|-|\n> | uno‸ | dos |"),
    (
        "sangrada en una lista",
        "- ítem\n\n    | a | b |\n    |-|-|\n    | uno‸ | dos |",
    ),
    (
        "el párrafo de arriba no es la tabla",
        "texto | suelto\n| a | b |\n|-|-|\n| uno‸ | dos |",
    ),
    (
        "termina en la línea sin pipe",
        "| a | b |\n|-|-|\n| uno‸ | dos |\nfin\n| no | es |",
    ),
    (
        "solo la del cursor",
        "| a | b |\n|-|-|\n| uno | dos |\n\n| c |\n|-|\n| tres‸ |",
    ),
    (
        "ya alineada no cambia",
        "| a   | b   |\n| --- | --- |\n| uno‸ | dos |",
    ),
    (
        "no es tabla: la separadora no cuadra",
        "| a | b |\n|-|\n| uno‸ | dos |",
    ),
    ("no es tabla: sin separadora", "| a | b |\n| uno‸ | dos |"),
    (
        "no es tabla: el cursor arriba del encabezado",
        "x | y‸\n\n| a | b |\n|-|-|",
    ),
    ("no es tabla: una regla", "---‸"),
    (
        "los espacios duros son contenido",
        "| a | b |\n|-|-|\n|\u{a0}uno‸\u{a0}| \u{3000}備考 |",
    ),
    (
        "termina donde empieza otro bloque",
        "| a | b |\n|-|-|\n| uno‸ | dos |\n- ítem | con pipe\n| no | es |",
    ),
    (
        "termina en un heading",
        "| a | b |\n|-|-|\n| uno‸ | dos |\n# título | con pipe",
    ),
    (
        "termina en una cerca",
        "| a | b |\n|-|-|\n| uno‸ | dos |\n```a|b\n| no | es |\n```",
    ),
    (
        "termina al salir de la cita",
        "> | a | b |\n> |-|-|\n> | uno‸ | dos |\nfuera | de la cita",
    ),
    (
        "termina en una cita más honda",
        "> | a | b |\n> |-|-|\n> | uno‸ | dos |\n> > | otra | cita |",
    ),
    (
        "no entra en la cita de abajo",
        "| a | b |\n|-|-|\n| uno‸ | dos |\n> | otra | cita |",
    ),
    (
        "separadoras cortas, como prettier",
        "| a | b | c |\n|:-|:-:|-:|\n| x‸ | y | z |",
    ),
];

/// La nota con la tabla del cursor alineada y el cursor donde queda.
fn formatted(case: &str) -> String {
    let at = case.find(CURSOR).unwrap();
    let text = case.replacen(CURSOR, "", 1);
    let line = text[..at].matches('\n').count();
    let column = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    let lines: Vec<&str> = text.split('\n').collect();
    let Some(table) = Table::around(&TextLines::new(&text), line) else {
        return "(sin tabla)\n".to_owned();
    };
    let formatted = table.formatted();
    let (row, byte) = formatted.place(table.spot(line, column));
    let mut out = String::new();
    for (index, original) in lines.iter().enumerate() {
        let mut shown = match index.checked_sub(table.lines().start) {
            Some(r) if table.lines().contains(&index) => formatted.lines()[r].clone(),
            _ => (*original).to_owned(),
        };
        if index == table.lines().start + row {
            shown.insert(byte, CURSOR);
        }
        writeln!(out, "{shown}").unwrap();
    }
    out
}

#[test]
fn formats_the_table_under_the_cursor() {
    let mut out = String::new();
    for (name, case) in CASES {
        writeln!(out, "── {name}\n{case}\n▼\n{}", formatted(case)).unwrap();
    }
    insta::assert_snapshot!(out);
}

#[test]
fn formatting_twice_changes_nothing() {
    for (name, case) in CASES
        .iter()
        .filter(|(name, _)| !name.starts_with("no es tabla"))
    {
        let once = formatted(case);
        assert_eq!(formatted(once.trim_end()), once, "{name}");
    }
}

#[test]
fn a_carriage_return_is_not_part_of_the_last_cell() {
    let text = "| a | b |\r\n|-|-:|\r\n| uno | dos |\r";
    let table = Table::around(&TextLines::new(text), 2).unwrap();
    assert_eq!(table.aligns, [Align::None, Align::Right]);
    assert_eq!(
        table.formatted().lines(),
        ["| a   |   b |", "| --- | --: |", "| uno | dos |"]
    );
}

#[test]
fn spots_name_the_cell_and_the_offset_in_it() {
    let text = "| ab | cd |\n|-|-|";
    let table = Table::around(&TextLines::new(text), 0).unwrap();
    let spot = |row, column, offset| CellSpot {
        cell: CellPos { row, column },
        offset,
    };
    assert_eq!(table.spot(0, 0), spot(0, 0, 0));
    assert_eq!(table.spot(0, 3), spot(0, 0, 1));
    assert_eq!(table.spot(0, 5), spot(0, 0, 2));
    assert_eq!(table.spot(0, 8), spot(0, 1, 1));
    assert_eq!(table.spot(1, 3), spot(1, 1, 0));
}

#[test]
fn steps_walk_the_cells_in_reading_order_past_the_separator() {
    let text = "| a | b |\n|-|-|\n| 1 |\n| 1 | 2 | 3 |";
    let table = Table::around(&TextLines::new(text), 0).unwrap();
    let cell = |row, column| Some(CellPos { row, column });
    let cases = [
        ((0, 0), Step::Next, cell(0, 1)),
        ((0, 1), Step::Next, cell(2, 0)),
        // la fila corta se alinea con tantas celdas como el encabezado
        ((2, 0), Step::Next, cell(2, 1)),
        ((2, 1), Step::Next, cell(3, 0)),
        // y la larga conserva las de más
        ((3, 1), Step::Next, cell(3, 2)),
        ((3, 2), Step::Next, None),
        ((1, 0), Step::Next, cell(2, 0)),
        ((1, 1), Step::Next, cell(2, 0)),
        ((3, 0), Step::Previous, cell(2, 1)),
        ((2, 0), Step::Previous, cell(0, 1)),
        ((1, 1), Step::Previous, cell(0, 1)),
        ((0, 1), Step::Previous, cell(0, 0)),
        ((0, 0), Step::Previous, cell(0, 0)),
    ];
    for ((row, column), step, expected) in cases {
        assert_eq!(
            table.step(CellPos { row, column }, step),
            expected,
            "{step:?} desde ({row}, {column})"
        );
    }
}

#[test]
fn a_new_row_has_as_many_empty_cells_as_the_header() {
    let text = "> | nombre | x |\n> |:-:|-:|\n> | José | 1 | sobra |";
    let table = Table::around(&TextLines::new(text), 0).unwrap();
    let formatted = table.formatted_with_new_row();
    assert_eq!(
        formatted.lines(),
        [
            "> | nombre |   x |",
            "> | :----: | --: |",
            "> |  José  |   1 | sobra |",
            "> |        |     |",
        ]
    );
    let first = CellSpot {
        cell: CellPos { row: 3, column: 0 },
        offset: 0,
    };
    assert_eq!(formatted.place(first), (3, 7));
}

/// Lo que dice cada celda de cada fila de la tabla, en orden.
fn contents(table: &Table) -> Vec<Vec<String>> {
    table
        .rows
        .iter()
        .zip(&table.lines)
        .map(|(row, line)| {
            row.cells
                .iter()
                .map(|cell| line[cell.content.clone()].to_owned())
                .filter(|content| !content.is_empty())
                .collect()
        })
        .collect()
}

/// Una tabla escrita a mano: celdas del cuerpo con lo que suele romper un
/// partidor, separadoras de cualquier largo y pipes de los bordes que pueden faltar.
fn written_table() -> impl Strategy<Value = String> {
    let cell = "[ a:\\-|\\\\`$\\[\\]é東]{0,6}";
    let header = "[a:é東][ a:é東]{0,5}";
    let delimiter = "[ ]{0,2}:?-{1,4}:?";
    (1usize..4).prop_flat_map(move |columns| {
        (
            prop::collection::vec(header, columns),
            prop::collection::vec(delimiter, columns),
            prop::collection::vec(prop::collection::vec(cell, 0..6), 0..4),
            any::<bool>(),
        )
            .prop_map(|(header, delimiter, body, edges)| {
                let row = |cells: &[String]| {
                    let inner = cells.join("|");
                    if edges {
                        format!("|{inner}|")
                    } else {
                        format!("{inner}|")
                    }
                };
                let mut lines = vec![row(&header), row(&delimiter)];
                lines.extend(body.iter().map(|cells| row(cells)));
                lines.join("\n")
            })
    })
}

proptest! {
    #[test]
    fn formatting_keeps_every_cell_and_settles(
        text in written_table(),
        line in any::<prop::sample::Index>(),
        column in 0usize..24,
    ) {
        let table = Table::around(&TextLines::new(&text), 0).unwrap();
        let line = line.index(table.lines().end);
        let formatted = table.formatted();
        let original = text.split('\n').nth(line).unwrap_or("");
        let at = original.len().min(column);
        if original.is_char_boundary(at) {
            let (row, byte) = formatted.place(table.spot(line, at));
            prop_assert!(formatted.lines()[row].is_char_boundary(byte));
            prop_assert_eq!(table.lines().start + row, line);
        }
        let again_text = formatted.lines().join("\n");
        let again = Table::around(&TextLines::new(&again_text), 0).unwrap();
        prop_assert_eq!(&contents(&again)[2..], &contents(&table)[2..]);
        prop_assert_eq!(again.formatted(), formatted);
    }
}
