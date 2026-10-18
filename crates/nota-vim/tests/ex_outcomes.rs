#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Lo que los comandos de `:` le cuentan a la UI: errores, avisos y la
//! línea que deja `:` en Visual. Lo que hacen con el texto está en la tabla
//! de `ex.rs`, que también corre en nvim.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use nota_vim::{
    Action, CommandError, GlobalMiss, KeyOutcome, Mode, SearchError, Substitution, Vim,
};
use pretty_assertions::assert_eq;
use rstest::rstest;

fn run(before: &str, keys: &str) -> (KeyOutcome, Vim, String) {
    let (mut buffer, mut selections) = support::parse(before);
    let mut vim = Vim::new();
    let last = support::feed(&mut vim, &mut buffer, &mut selections, keys);
    let text = support::render(&buffer, selections.iter().map(|s| s.head));
    (last.unwrap(), vim, text)
}

const TEXT: &str = "a|b\ncd\nab";

#[rstest]
#[case(":m<CR>", CommandError::InvalidAddress)]
#[case(":m9<CR>", CommandError::InvalidAddress)]
#[case(":1,2m1<CR>", CommandError::MoveIntoItself)]
#[case(":2,1d<CR>", CommandError::BackwardsRange)]
#[case(":4d<CR>", CommandError::InvalidRange)]
#[case(":-2d<CR>", CommandError::InvalidRange)]
#[case(":'a<CR>", CommandError::MarkNotSet)]
#[case(":'<lt>d<CR>", CommandError::MarkNotSet)]
#[case(":/zz/d<CR>", CommandError::Search(SearchError::NotFound("zz".to_owned())))]
#[case("://d<CR>", CommandError::Search(SearchError::NoPrevious))]
#[case(":d!<CR>", CommandError::NoBangAllowed)]
#[case(":d 0<CR>", CommandError::PositiveCountRequired)]
#[case(":d x y<CR>", CommandError::TrailingCharacters("y".to_owned()))]
#[case(":1noh<CR>", CommandError::NoRangeAllowed)]
#[case(":norm<CR>", CommandError::ArgumentRequired)]
#[case(":sort x<CR>", CommandError::UnsupportedSortFlag('x'))]
#[case(":sort z<CR>", CommandError::InvalidArgument)]
#[case(":sort /a<CR>", CommandError::MissingDelimiter("/a".to_owned()))]
#[case(":g<CR>", CommandError::GlobalPatternMissing)]
#[case(":gxaxd<CR>", CommandError::Unknown("gxaxd".to_owned()))]
#[case(":g xaxd<CR>", CommandError::LetterDelimiter)]
#[case(":g/a/2,3g/b/d<CR>", CommandError::RecursiveGlobal)]
#[case(":g/a/.,+1m1<CR>", CommandError::MoveIntoItself)]
#[case(":g/a/w<CR>", CommandError::HostCommandInside("w".to_owned()))]
#[case(":norm x p<CR>", CommandError::HostCommandInside("workspace::open_command_palette".to_owned()))]
fn reports_why_a_command_failed(#[case] keys: &str, #[case] error: CommandError) {
    let (outcome, _, _) = run(TEXT, keys);
    assert_eq!(outcome, KeyOutcome::InvalidCommand(error));
}

#[rstest]
#[case(CommandError::Unknown("dd".to_owned()), "E492: Not an editor command: dd")]
#[case(CommandError::InvalidRange, "E16: Invalid range")]
#[case(CommandError::BackwardsRange, "E493: Backwards range given")]
#[case(CommandError::MarkNotSet, "E20: Mark not set")]
#[case(
    CommandError::MoveIntoItself,
    "E134: Cannot move a range of lines into itself"
)]
#[case(
    CommandError::Search(SearchError::NotFound("x".to_owned())),
    "E486: Pattern not found: x"
)]
#[case(
    CommandError::Search(SearchError::NoPrevious),
    "E35: No previous regular expression"
)]
fn vim_errors_keep_their_original_text(#[case] error: CommandError, #[case] text: &str) {
    assert_eq!(error.to_string(), text);
}

#[test]
fn a_failed_command_leaves_the_text_alone() {
    let (_, _, text) = run(TEXT, ":g/a/.,+1m1<CR>");
    assert_eq!(text, "|ab\ncd\nab");
    let (_, _, text) = run(TEXT, ":1,2m1<CR>");
    assert_eq!(text, TEXT);
}

#[rstest]
#[case(":g/zz/d<CR>", "zz", false, "Pattern not found: zz")]
#[case(":v/./d<CR>", ".", true, "Pattern found in every line: .")]
fn a_global_without_lines_is_a_notice(
    #[case] keys: &str,
    #[case] pattern: &str,
    #[case] inverted: bool,
    #[case] message: &str,
) {
    let (outcome, _, text) = run(TEXT, keys);
    let miss = GlobalMiss {
        pattern: pattern.to_owned(),
        inverted,
    };
    assert_eq!(miss.to_string(), message);
    assert_eq!(outcome, KeyOutcome::GlobalMissed(miss));
    assert_eq!(text, TEXT);
}

#[test]
fn a_global_reports_every_substitution_together() {
    let (outcome, _, _) = run("|a a\nb\na", ":g/a/s/a/x/g<CR>");
    assert_eq!(
        outcome,
        KeyOutcome::Substituted(Substitution {
            replacements: 3,
            lines: 2
        })
    );
    let (outcome, _, _) = run("|a a\nb\na", ":g/b/s/a/x/g<CR>");
    assert_eq!(outcome, KeyOutcome::Executed(Action::SubmitCommand));
}

#[test]
fn colon_in_visual_starts_with_the_selection() {
    let (_, vim, _) = run(TEXT, "Vj:");
    assert_eq!(vim.mode(), Mode::CommandLine);
    assert_eq!(vim.command_line(), Some("'<,'>"));
}

/// Como vim: `@:` sin una línea anterior falla y corta la macro.
#[test]
fn repeating_without_a_previous_line_fails() {
    let (_, _, text) = run("|abc", "qqx@:xq@q");
    assert_eq!(text, "|");
    let (_, _, text) = run("|abcd", "qq@:xq0@q");
    assert_eq!(text, "|bcd");
}

/// nvim no tiene multi-cursor: los comandos de `:` usan el cursor principal
/// y dejan uno solo.
#[test]
fn ex_commands_leave_a_single_cursor() {
    let (_, _, text) = run("|ab\n|cd\nef", ":3d<CR>");
    assert_eq!(text, "ab\n|cd");
}
