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

use metalmind_vim::{
    Action, CommandError, GlobalMiss, KeyOutcome, Mode, SearchError, Substitution, Vim,
};
use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};

fn run(before: &str, keys: &str) -> (KeyOutcome, Vim, String) {
    let (mut buffer, mut selections) = support::parse(before);
    let mut vim = Vim::new();
    let last = support::feed(&mut vim, &mut buffer, &mut selections, keys);
    let text = support::render(&buffer, selections.iter().map(|s| s.head));
    (last.unwrap(), vim, text)
}

const TEXT: &str = "a|b\ncd\nab";

#[rstest]
#[case(":m<CR>", CommandError::InvalidRange)]
#[case(":m9<CR>", CommandError::InvalidRange)]
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
#[case(":sort l<CR>", CommandError::UnsupportedSortFlag('l'))]
#[case(":sort z<CR>", CommandError::InvalidArgumentAt("z".to_owned()))]
#[case(":sort nx<CR>", CommandError::InvalidArgument)]
#[case(":sort /a<CR>", CommandError::MissingDelimiter("a".to_owned()))]
#[case(":g<CR>", CommandError::GlobalPatternMissing)]
#[case(":gxaxd<CR>", CommandError::Unknown("gxaxd".to_owned()))]
#[case(":g xaxd<CR>", CommandError::LetterDelimiter)]
#[case(":g/a/2,3g/b/d<CR>", CommandError::RecursiveGlobal)]
#[case(":g/a/.,+1m1<CR>", CommandError::MoveIntoItself)]
#[case(":g/a/w<CR>", CommandError::HostCommandInside("w".to_owned()))]
#[case(":1,2w<CR>", CommandError::PartialWrite)]
#[case("Vj:wq<CR>", CommandError::PartialWrite)]
#[case(":1,2w!<CR>", CommandError::UnsupportedRange("w!".to_owned()))]
#[case(":2q<CR>", CommandError::UnsupportedRange("q".to_owned()))]
#[case(":'<lt>,'>vs<CR>", CommandError::UnsupportedRange("vs".to_owned()))]
#[case(":delm<CR>", CommandError::ArgumentRequired)]
#[case(":delm! a<CR>", CommandError::InvalidArgument)]
#[case(":delm b-a<CR>", CommandError::InvalidArgumentAt("b-a".to_owned()))]
#[case(":1delm a<CR>", CommandError::NoRangeAllowed)]
#[case(":delm a<lt><CR>", CommandError::UnsupportedDeleteMark('<'))]
#[case(":delm >a<CR>", CommandError::UnsupportedDeleteMark('>'))]
#[case(":norm x p<CR>", CommandError::HostCommandInside("workspace::open_command_palette".to_owned()))]
fn reports_why_a_command_failed(#[case] keys: &str, #[case] error: CommandError) {
    let (outcome, _, _) = run(TEXT, keys);
    assert_eq!(outcome, KeyOutcome::InvalidCommand(error));
}

/// El texto que la UI muestra en la barra tras un comando.
fn message(outcome: &KeyOutcome) -> String {
    match outcome {
        KeyOutcome::InvalidCommand(error) => error.to_string(),
        KeyOutcome::SearchFailed(error) => error.to_string(),
        KeyOutcome::GlobalMissed(miss) => miss.to_string(),
        KeyOutcome::Substituted(substitution) => substitution.report().unwrap_or_default(),
        _ => String::new(),
    }
}

/// Los mensajes de vim, con su número y su texto en inglés, tal como los
/// deja nvim.
#[template]
#[rstest]
#[case(":dd<CR>", "E492: Not an editor command: dd")]
#[case(":m<CR>", "E16: Invalid range")]
#[case(":m9<CR>", "E16: Invalid range")]
#[case(":m x<CR>", "E16: Invalid range")]
#[case(":t-5<CR>", "E16: Invalid range")]
#[case(":4d<CR>", "E16: Invalid range")]
#[case(":-2d<CR>", "E16: Invalid range")]
#[case(":1,2m1<CR>", "E134: Cannot move a range of lines into itself")]
#[case(":'a<CR>", "E20: Mark not set")]
#[case(":'<lt>d<CR>", "E20: Mark not set")]
#[case(":/zz/d<CR>", "E486: Pattern not found: zz")]
#[case("://d<CR>", "E35: No previous regular expression")]
#[case("/zz<CR>", "E486: Pattern not found: zz")]
#[case(":d!<CR>", "E477: No ! allowed")]
#[case(":d 0<CR>", "E939: Positive count required")]
#[case(":d x y<CR>", "E488: Trailing characters: y")]
#[case(":1noh<CR>", "E481: No range allowed")]
#[case(":norm<CR>", "E471: Argument required")]
#[case(":sort zq<CR>", "E475: Invalid argument: zq")]
#[case(":sort /a/ /b/<CR>", "E475: Invalid argument: /b/")]
#[case(":sort nx<CR>", "E474: Invalid argument")]
#[case(":sort /a<CR>", "E654: Missing delimiter after search pattern: a")]
#[case(":g<CR>", "E148: Regular expression missing from global")]
#[case(":gxaxd<CR>", "E492: Not an editor command: gxaxd")]
#[case(
    ":g xaxd<CR>",
    "E146: Regular expressions can't be delimited by letters"
)]
#[case(":g/a/2,3g/b/d<CR>", "E147: Cannot do :global recursive with a range")]
#[case(":g/a/.,+1m1<CR>", "E134: Cannot move a range of lines into itself")]
#[case(":g/a/zz<CR>", "E492: Not an editor command: zz")]
#[case(":g/zz/d<CR>", "Pattern not found: zz")]
#[case(":v/./d<CR>", "Pattern found in every line: .")]
#[case(":2d<CR>", "")]
#[case(":1,2zz<CR>", "E492: Not an editor command: 1,2zz")]
#[case(":1,2e x<CR>", "E481: No range allowed")]
#[case(":2set nu<CR>", "E481: No range allowed")]
#[case("@:", "E30: No previous command line")]
#[case(":norm @:<CR>", "E30: No previous command line")]
#[case(":norm @:<CR>:norm @:<CR>", "E169: Command too recursive")]
#[case("o<Esc>:g/^/normal *<CR>", "E348: No string under cursor")]
#[case(":%s/./x/g<CR>", "6 substitutions on 3 lines")]
#[case(":g/./s/^/x/<CR>", "3 substitutions on 3 lines")]
#[case(":%j<CR>:s/[ab]/x/g<CR>", "4 substitutions on 1 line")]
#[case(":%s/a/x/<CR>", "")]
#[case(":delm<CR>", "E471: Argument required")]
#[case(":delm \"a<CR>", "E471: Argument required")]
#[case(":delm! a<CR>", "E474: Invalid argument")]
#[case(":delm b-a<CR>", "E475: Invalid argument: b-a")]
#[case(":delm a-Z<CR>", "E475: Invalid argument: a-Z")]
#[case(":delm a-<CR>", "E475: Invalid argument: a-")]
#[case(":delm ab?c<CR>", "E475: Invalid argument: ?c")]
#[case(":delm '<CR>", "E475: Invalid argument: '")]
#[case(":1delm a<CR>", "E481: No range allowed")]
#[case(":delm a 0-9 ^.[]<CR>", "")]
#[case(":delmarks a<CR>", "")]
fn messages(#[case] keys: &str, #[case] text: &str) {}

#[apply(messages)]
fn engine_message(#[case] keys: &str, #[case] text: &str) {
    let (outcome, _, _) = run(TEXT, keys);
    assert_eq!(message(&outcome), text);
}

#[apply(messages)]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_message(#[case] keys: &str, #[case] text: &str) {
    let nvim = support::nvim::message(TEXT, keys);
    assert_eq!(nvim, text, "la tabla no coincide con nvim");
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

/// Como vim, una línea de `:` que se repite a sí misma corta a los 200
/// niveles con `E169`, y lo que alcanzó a hacer queda.
const RECURSIVE: &str = "300ix<Esc>0:norm x@:<CR>:norm x@:<CR>";

#[test]
fn a_command_line_that_repeats_itself_stops() {
    let (outcome, _, text) = run("|", RECURSIVE);
    assert_eq!(message(&outcome), "E169: Command too recursive");
    assert_eq!(text.len(), 300 - 201 + 1, "{text}");
}

#[test]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn a_command_line_that_repeats_itself_stops_like_nvim() {
    let (_, _, text) = run("|", RECURSIVE);
    assert_eq!(support::nvim::run("|", RECURSIVE).text, text);
}
