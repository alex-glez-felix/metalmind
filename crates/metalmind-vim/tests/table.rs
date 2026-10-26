#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! `editor::format_table` y `:tableformat` en el motor: el undo, `.`, los
//! cursores y dónde no hay tabla. nvim no alinea tablas, así que no hay
//! oráculo; qué es una tabla y cómo queda alineada se prueba en
//! `metalmind-syntax`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_buffer::{ByteOffset, Selection, Selections};
use metalmind_syntax::Rows;
use metalmind_vim::{Action, CommandError, DocState, KeyOutcome};
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
