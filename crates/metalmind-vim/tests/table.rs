#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Las tablas en el motor: `editor::format_table` y `:tableformat`, `tab` y
//! `shift-tab` entre celdas en Insert y alinear al salir de Insert, con el
//! undo, `.`, el count y los cursores. nvim no alinea tablas, así que no hay
//! oráculo (fuera de una tabla `tab` y `shift-tab` escriben un tab, y eso
//! sí se compara con nvim en `vim.rs`); qué es una tabla y cómo queda
//! alineada se prueba en `metalmind-syntax`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_buffer::{ByteOffset, Selection, Selections};
use metalmind_syntax::Rows;
use metalmind_vim::{Action, CommandError, DocState, KeyOutcome, Markdown};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// El cursor en estos casos: `|` es de la tabla.
const CURSOR: char = '‸';

/// `before` con sus cursores, con el motor y los `:` de la app y el estilo
/// de las líneas que comparte con el editor.
fn editor(before: &str) -> support::Editor {
    let mut editor = support::Editor::app("|");
    let mut text = String::new();
    let mut cursors = Vec::new();
    for c in before.chars() {
        if c == CURSOR {
            cursors.push(Selection::cursor(ByteOffset::new(text.len())));
        } else {
            text.push(c);
        }
    }
    editor.buffer = text.as_str().into();
    editor.selections = Selections::new(cursors).unwrap();
    editor.doc = DocState::painted(&editor.buffer, Rows::uniform(1));
    editor
}

/// El texto con cada cursor.
fn shown(editor: &support::Editor) -> String {
    let mut text = editor.text();
    let mut heads: Vec<usize> = editor.selections.iter().map(|s| s.head.get()).collect();
    heads.sort_unstable();
    for head in heads.into_iter().rev() {
        text.insert(head, CURSOR);
    }
    text
}

/// Como `support::Editor::keys`, con `<format>` como `editor::format_table`,
/// que no tiene tecla.
fn run(before: &str, keys: &str) -> String {
    let mut editor = editor(before);
    let format = Action::from_name("editor::format_table").unwrap();
    let mut pieces = keys.split("<format>").peekable();
    while let Some(piece) = pieces.next() {
        editor.keys(piece);
        if pieces.peek().is_some() {
            editor.execute(format);
        }
    }
    shown(&editor)
}

const ROUGH: &str = "| a | b |\n|-|-|\n| un‸o | dos |";
const ALIGNED: &str = "| a   | b   |\n| --- | --- |\n| un‸o | dos |";

#[rstest]
#[case(ROUGH, "<format>", ALIGNED)]
#[case(ROUGH, ":tableformat<CR>", ALIGNED)]
#[case(ROUGH, ":tablef<CR>", ALIGNED)]
// un solo paso de undo, y el redo vuelve a dejarla alineada
#[case(ROUGH, "<format>u", "| a | b |\n|-|-|\n| un‸o | dos |")]
#[case(
    ROUGH,
    "<format>u<C-r>",
    "| a   | b   |\n| --- ‸| --- |\n| uno | dos |"
)]
#[case(ROUGH, ":tableformat<CR>u", "| a | b |\n|-|-|\n| un‸o | dos |")]
// `.` alinea la tabla donde esté el cursor
#[case(
    "| a |\n|-|\n| u‸no |\n\n| b |\n|-|\n| dos |",
    "<format>G.",
    "| a   |\n| --- |\n| uno |\n\n| b   |\n| --- |\n| d‸os |"
)]
// una tabla ya alineada no cambia: `u` deshace lo de antes
#[case(ALIGNED, "rX<format>u", ALIGNED)]
// sin tabla, en código o en el frontmatter, nada cambia
#[case("a | b‸", "<format>", "a | b‸")]
#[case(
    "```\n| a | b |\n|-|-|\n| u‸no | dos |\n```",
    "<format>",
    "```\n| a | b |\n|-|-|\n| u‸no | dos |\n```"
)]
#[case(
    "---\nx: | a |\n‸y: |-|\n---",
    "<format>",
    "---\nx: | a |\n‸y: |-|\n---"
)]
// un archivo CRLF conserva sus `\r`
#[case(
    "| a | b |\r\n|-|-|\r\n| un‸o | dos |\r\n",
    "<format>",
    "| a   | b   |\r\n| --- | --- |\r\n| un‸o | dos |\r\n"
)]
// cada cursor en su celda, dos en una misma tabla o en tablas distintas
#[case(
    "| ‸a | b |\n|-|-|\n| uno | ‸dos |\n\n| c‸ |\n|-|\n‸fin",
    "<format>",
    "| ‸a   | b   |\n| --- | --- |\n| uno | ‸dos |\n\n| c‸   |\n| --- |\n‸fin"
)]
fn formats_the_table_under_the_cursor(
    #[case] before: &str,
    #[case] keys: &str,
    #[case] after: &str,
) {
    assert_eq!(run(before, keys), after);
}

#[test]
fn format_table_has_a_name() {
    assert_eq!(Action::FormatTable.name(), "editor::format_table");
    assert_eq!(
        Action::from_name("editor::format_table"),
        Some(Action::FormatTable)
    );
}

#[rstest]
#[case("a‸", ":tableformat<CR>", CommandError::NotInTable)]
#[case(ROUGH, ":%tableformat<CR>", CommandError::NoRangeAllowed)]
#[case(ROUGH, ":tableformat!<CR>", CommandError::NoBangAllowed)]
#[case(ROUGH, ":tableformat x<CR>", CommandError::TrailingCharacters("x".to_owned()))]
fn tableformat_says_why_it_did_nothing(
    #[case] before: &str,
    #[case] keys: &str,
    #[case] error: CommandError,
) {
    let mut editor = editor(before);
    assert_eq!(editor.keys(keys), Some(KeyOutcome::InvalidCommand(error)));
    assert_eq!(shown(&editor), before);
}

#[rstest]
// alinea y salta al principio del contenido de la celda siguiente
#[case(ROUGH, "i<Tab>", "| a   | b   |\n| --- | --- |\n| uno | ‸dos |")]
// del encabezado al cuerpo, sin pasar por la separadora
#[case(
    "| a | ‸b |\n|-|-|\n| uno | dos |",
    "i<Tab>",
    "| a   | b   |\n| --- | --- |\n| ‸uno | dos |"
)]
#[case(
    "| a | b |\n|-‸|-|\n| uno | dos |",
    "i<Tab>",
    "| a   | b   |\n| --- | --- |\n| ‸uno | dos |"
)]
// tras la última celda, una fila con tantas celdas como el encabezado
#[case(
    "| a | b |\n|-|-|\n| uno | do‸s |",
    "i<Tab>",
    "| a   | b   |\n| --- | --- |\n| uno | dos |\n| ‸    |     |"
)]
#[case(
    "| a | b |\n|-|-|\n| uno | do‸s |",
    "i<Tab>x<Tab>y<Esc>",
    "| a   | b   |\n| --- | --- |\n| uno | dos |\n| x   | ‸y    |"
)]
#[case(
    "> | a |\n> |-|\n> | x‸ |\nfin",
    "i<Tab>",
    "> | a   |\n> | --- |\n> | x   |\n> | ‸    |\nfin"
)]
#[case(
    "| a |\r\n|-|\r\n| u‸no |\r\n",
    "i<Tab>",
    "| a   |\r\n| --- |\r\n| uno |\r\n| ‸    |\r\n"
)]
// `shift-tab` vuelve; en la primera celda se queda
#[case(
    "| a | b |\n|-|-|\n| uno | ‸dos |",
    "i<S-Tab>",
    "| a   | b   |\n| --- | --- |\n| ‸uno | dos |"
)]
#[case(
    "| a | b |\n|-|-|\n| ‸uno | dos |",
    "i<S-Tab>",
    "| a   | ‸b   |\n| --- | --- |\n| uno | dos |"
)]
#[case("| ‸a | b |\n|-|-|", "i<S-Tab>", "| ‸a   | b   |\n| --- | --- |")]
// fuera de una tabla, o en un bloque de código, escriben un tab
#[case("a‸b", "i<Tab><S-Tab>", "a\t\t‸b")]
#[case(
    "```\n| a |\n|-|\n| u‸no |\n```",
    "i<Tab>",
    "```\n| a |\n|-|\n| u\t‸no |\n```"
)]
// un solo paso de undo con lo escrito, y vuelve donde empezó
#[case(ROUGH, "i<Tab>x<Esc>u", ROUGH)]
#[case(
    "| a | b |\n|-|-|\n| uno | do‸s |",
    "i<Tab><Esc>u",
    "| a | b |\n|-|-|\n| uno | do‸s |"
)]
// el count y `.` lo repiten como tecla, no como el texto que dejó
#[case(
    "| ‸a | b | c |\n|-|-|-|\n| 1 | 2 | 3 |",
    "3i<Tab><Esc>",
    "| a   | b   | c   |\n| --- | --- | --- |\n|‸ 1   | 2   | 3   |"
)]
#[case(
    "| ‸a | b |\n|-|-|\n\n| c | d |\n|-|-|\n| tres | cuatro |",
    "i<Tab><Esc>G0.",
    "| a   | b   |\n| --- | --- |\n\n| c    | d      |\n| ---- | ------ |\n| tres |‸ cuatro |"
)]
// cada cursor salta en su tabla; dos tras la última celda, una sola fila
#[case(
    "| ‸a | b |\n|-|-|\n| uno | dos‸ |\n\nx‸",
    "i<Tab>",
    "| a   | ‸b   |\n| --- | --- |\n| uno | dos |\n| ‸    |     |\n\nx\t‸"
)]
#[case(
    "| a | b‸ |\n|-|-|\n| uno | dos‸ |",
    "i<Tab><Tab>",
    "| a   | b   |\n| --- | --- |\n| uno | ‸dos |\n|     | ‸    |"
)]
fn tab_moves_between_cells(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(run(before, keys), after);
}

#[test]
fn the_cell_actions_have_names() {
    for (action, name) in [
        (Action::NextTableCell, "editor::next_table_cell"),
        (Action::PreviousTableCell, "editor::previous_table_cell"),
    ] {
        assert_eq!(action.name(), name);
        assert_eq!(Action::from_name(name), Some(action));
    }
}

/// Como [`run`], con `align_tables_on_leave`.
fn run_aligning_on_leave(before: &str, keys: &str) -> String {
    let mut editor = editor(before);
    editor.vim.set_markdown(Markdown {
        align_tables_on_leave: true,
        ..Markdown::default()
    });
    editor.keys(keys);
    shown(&editor)
}

const TYPED: &str = "| a   | b   |\n| --- | --- |\n| ‸x   | y   |";

#[rstest]
#[case(TYPED, "iabc<Esc>", "| a    | b   |\n| ---- | --- |\n| ab‸cx | y   |")]
// en el mismo paso de undo que lo escrito
#[case(TYPED, "iabc<Esc>u", TYPED)]
// `.` lo repite, también la alineación
#[case(
    "| ‸a | b |\n|-|-|\n\n| c | d |\n|-|-|\n| tres | cuatro |",
    "ix<Esc>G0w.",
    "| xa  | b   |\n| --- | --- |\n\n| c     | d      |\n| ----- | ------ |\n| ‸xtres | cuatro |"
)]
// con el count, alinea una vez al final
#[case(
    TYPED,
    "3iab<Esc>",
    "| a       | b   |\n| ------- | --- |\n| ababa‸bx | y   |"
)]
// si no cambió nada, o se sale con `ctrl-c`, no alinea
#[case(ROUGH, "i<Esc>", "| a | b |\n|-|-|\n| u‸no | dos |")]
#[case(ROUGH, "iz<C-c>", "| a | b |\n|-|-|\n| un‸zo | dos |")]
// fuera de una tabla no toca nada
#[case("| a | b |\n|-|-|\n\n‸fin", "ix<Esc>", "| a | b |\n|-|-|\n\n‸xfin")]
fn leaving_insert_aligns_the_table(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(run_aligning_on_leave(before, keys), after);
}

#[test]
fn leaving_insert_does_not_align_by_default() {
    assert_eq!(
        run(TYPED, "iabc<Esc>"),
        "| a   | b   |\n| --- | --- |\n| ab‸cx   | y   |"
    );
}
