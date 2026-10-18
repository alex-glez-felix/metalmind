#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Lo que la búsqueda le cuenta a la UI, y lo que nvim no puede
//! comparar: multi-cursor y donde nota se aparta de vim a propósito. El
//! movimiento y la edición están en la tabla de `search.rs`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use nota_buffer::ByteOffset;
use nota_vim::{Action, KeyOutcome, Prompt, SearchError, Vim, Wrap};
use pretty_assertions::assert_eq;
use rstest::rstest;

/// Corre `keys` sobre `before` y devuelve lo que pasó con la última tecla,
/// el motor y el texto con los cursores.
fn run(before: &str, keys: &str) -> (KeyOutcome, Vim, String) {
    let (mut buffer, mut selections) = support::parse(before);
    let mut vim = Vim::new();
    let last = support::feed(&mut vim, &mut buffer, &mut selections, keys);
    let text = support::render(&buffer, selections.iter().map(|s| s.head));
    (last.unwrap(), vim, text)
}

fn outcome(before: &str, keys: &str) -> KeyOutcome {
    run(before, keys).0
}

fn not_found(pattern: &str) -> SearchError {
    SearchError::NotFound(pattern.to_owned())
}

#[rstest]
#[case("|a x", "/x<CR>", KeyOutcome::Executed(Action::SubmitCommand))]
#[case("a |x", "/x<CR>", KeyOutcome::SearchWrapped(Wrap::ToTop))]
#[case("|x a", "?x<CR>", KeyOutcome::SearchWrapped(Wrap::ToBottom))]
#[case("|x a x", "/x<CR>n", KeyOutcome::SearchWrapped(Wrap::ToTop))]
#[case("|x a x", "/x<CR>N", KeyOutcome::Executed(Action::SearchNext { reversed: true }))]
#[case("|x a x", "3/x<CR>", KeyOutcome::SearchWrapped(Wrap::ToTop))]
#[case("|a x a", "/x<CR>dn", KeyOutcome::SearchWrapped(Wrap::ToTop))]
#[case("a |foo", "*", KeyOutcome::SearchWrapped(Wrap::ToTop))]
#[case("|foo a foo", "#", KeyOutcome::SearchWrapped(Wrap::ToBottom))]
fn reports_when_a_search_wraps(
    #[case] before: &str,
    #[case] keys: &str,
    #[case] expected: KeyOutcome,
) {
    assert_eq!(outcome(before, keys), expected);
}

#[rstest]
#[case("|abc", "/zz<CR>", not_found("zz"))]
#[case("|abc", "d/zz<CR>", not_found("zz"))]
#[case("|abc", "/<CR>", SearchError::NoPrevious)]
#[case("|abc", "n", SearchError::NoPrevious)]
#[case("|abc", "dN", SearchError::NoPrevious)]
#[case("|  ", "*", SearchError::NoStringUnderCursor)]
#[case("|abc", r"/a\zsb<CR>", SearchError::Unsupported(r"\z".to_owned()))]
#[case("|abc", r"/a\nb<CR>", SearchError::Unsupported(r"\n".to_owned()))]
#[case("|abc", r"/\%Vb<CR>", SearchError::Unsupported(r"\%V".to_owned()))]
#[case("|abc", "/b/e<CR>", SearchError::Unsupported("/e".to_owned()))]
#[case("|abc", "?b?+1<CR>", SearchError::Unsupported("?+1".to_owned()))]
#[case("|abc", r"/a\{x}<CR>", SearchError::Invalid(r"a\{x}".to_owned()))]
#[case("|abc", r"/\(a<CR>", SearchError::Invalid(r"\(a".to_owned()))]
fn reports_why_a_search_failed(
    #[case] before: &str,
    #[case] keys: &str,
    #[case] error: SearchError,
) {
    assert_eq!(outcome(before, keys), KeyOutcome::SearchFailed(error));
}

#[test]
fn the_ui_can_read_the_prompt_and_what_was_typed() {
    let (_, vim, _) = run("|abc", "?ab");
    assert_eq!(
        vim.command_line_prompt(),
        Some(Prompt::Search { backward: true })
    );
    assert_eq!(vim.command_line(), Some("ab"));
    assert_eq!(Prompt::Search { backward: true }.symbol(), '?');
    assert_eq!(Prompt::Search { backward: false }.symbol(), '/');
    assert_eq!(Prompt::Command.symbol(), ':');
    let (_, vim, _) = run("|abc", ":w");
    assert_eq!(vim.command_line_prompt(), Some(Prompt::Command));
    let (_, vim, _) = run("|abc", "/ab<Esc>");
    assert_eq!(vim.command_line_prompt(), None);
}

#[test]
fn the_active_search_is_readable() {
    let (mut buffer, mut selections) = support::parse("|ab xab\nab");
    let mut vim = Vim::new();
    assert!(vim.search_highlight().is_none());
    support::feed(&mut vim, &mut buffer, &mut selections, "/ab<CR>");
    let pattern = vim.search_highlight().unwrap();
    assert_eq!(pattern.source(), "ab");
    let found: Vec<_> = pattern
        .matches_in(&buffer, 0..2)
        .into_iter()
        .map(|range| (range.start.get(), range.end.get()))
        .collect();
    assert_eq!(found, [(0, 2), (4, 6), (7, 9)]);
}

#[test]
fn search_actions_run_by_name() {
    let (mut buffer, mut selections) = support::parse("|a b c b");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "/b<CR>");
    let next = Action::from_name("vim::search_next").unwrap();
    vim.execute(next, &mut buffer, &mut selections).unwrap();
    assert_eq!(selections.primary().head, ByteOffset::new(6));
    let clear = Action::from_name("vim::clear_search_highlight").unwrap();
    vim.execute(clear, &mut buffer, &mut selections).unwrap();
    assert!(vim.search_highlight().is_none());
}

/// nvim no tiene multi-cursor: cada cursor busca desde donde está.
#[rstest]
#[case("|a x\n|b x", "/x<CR>", "a |x\nb |x")]
#[case("a |x\nb |x", "?x<CR>", "a |x\nb |x")]
#[case("|a x y\n|b x y", "d/y<CR>", "|y\n|y")]
#[case("fo|o x\nfoo |bar foo", "*", "foo x\n|foo bar |foo")]
fn multiple_cursors(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).text, after);
}

/// Donde nota se aparta de vim a propósito (ver el README del crate).
#[rstest]
#[case("|x ~a", "/~a<CR>", "x |~a")]
fn departs_from_vim_on_purpose(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).text, after);
}
