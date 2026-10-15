#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};

/// Cada caso es `(antes, teclas, después)`. El mismo caso corre en el motor
/// y en `nvim --clean`; si nvim no coincide con `después`, el caso está mal.
#[template]
#[rstest]
// h l
#[case("hel|lo world", "h", "he|llo world")]
#[case("|hello", "h", "|hello")]
#[case("hel|lo world", "3h", "|hello world")]
#[case("hel|lo", "9h", "|hello")]
#[case("h|ello", "l", "he|llo")]
#[case("hell|o", "l", "hell|o")]
#[case("|hello", "3l", "hel|lo")]
#[case("|hello", "9l", "hell|o")]
// j k y columna objetivo
#[case("|one\ntwo", "j", "one\n|two")]
#[case("one\n|two", "k", "|one\ntwo")]
#[case("|one\ntwo", "k", "|one\ntwo")]
#[case("one\n|two", "j", "one\n|two")]
#[case("lo|ng line\nab\nlong line", "jj", "long line\nab\nlo|ng line")]
#[case("long li|ne\nab\nlong line", "j", "long line\na|b\nlong line")]
#[case("|a\nb\nc\nd\ne\nf", "3j", "a\nb\nc\n|d\ne\nf")]
#[case("|a\nb\nc", "5j", "a\nb\n|c")]
#[case("a\nb\n|c", "5k", "|a\nb\nc")]
#[case("ab|c\n\nabc", "jj", "abc\n\nab|c")]
#[case("a|bc\nxy\nlonger", "$jj", "abc\nxy\nlonge|r")]
// w W
#[case("|foo bar baz", "w", "foo |bar baz")]
#[case("|foo bar baz", "2w", "foo bar |baz")]
#[case("|foo.bar baz", "w", "foo|.bar baz")]
#[case("|foo.bar baz", "W", "foo.bar |baz")]
#[case("foo |bar", "w", "foo ba|r")]
#[case("|foo\n\nbar", "w", "foo\n|\nbar")]
#[case("|foo\n  bar", "w", "foo\n  |bar")]
#[case("|foo   \nbar", "w", "foo   \n|bar")]
#[case("|añob c", "w", "añob |c")]
// b B
#[case("foo bar |baz", "b", "foo |bar baz")]
#[case("foo bar |baz", "2b", "|foo bar baz")]
#[case("foo.|bar", "b", "foo|.bar")]
#[case("foo.ba|r", "B", "|foo.bar")]
#[case("foo\n|bar", "b", "|foo\nbar")]
#[case("foo\n\n|bar", "b", "foo\n|\nbar")]
#[case("|foo", "b", "|foo")]
// e E
#[case("|foo bar", "e", "fo|o bar")]
#[case("fo|o bar", "e", "foo ba|r")]
#[case("|foo.bar baz", "E", "foo.ba|r baz")]
#[case("|foo.bar baz", "e", "fo|o.bar baz")]
#[case("|a b c d", "3e", "a b c |d")]
#[case("fo|o\n\nbar", "e", "foo\n\nba|r")]
#[case("|ñandú x", "e", "ñand|ú x")]
// 0 ^ $
#[case("  foo |bar", "0", "|  foo bar")]
#[case("  foo |bar", "^", "  |foo bar")]
#[case("|  foo bar", "^", "  |foo bar")]
#[case("fo|o bar", "$", "foo ba|r")]
#[case("|ab\ncd\nef", "2$", "ab\nc|d\nef")]
// gg G
#[case("ab\ncd\n|ef", "gg", "|ab\ncd\nef")]
#[case("a|b\ncd\nef", "G", "ab\ncd\ne|f")]
#[case("|ab\ncd\nef", "2G", "ab\n|cd\nef")]
#[case("ab\ncd\n|ef", "2gg", "ab\n|cd\nef")]
#[case("|ab\ncd", "9G", "ab\n|cd")]
#[case("  abc\nx|y", "gg", " | abc\nxy")]
// d con motions
#[case("hel|lo world", "dw", "hel|world")]
#[case("|foo bar", "dw", "|bar")]
#[case("foo |bar", "dw", "foo| ")]
#[case("foo |bar\nbaz", "dw", "foo| \nbaz")]
#[case("|foo   \nbar", "dw", "|\nbar")]
#[case("|foo\n\nbar", "dw", "|\n\nbar")]
#[case("foo\n|\nbar", "dw", "foo\n|bar")]
#[case("|a\nb", "dw", "|\nb")]
#[case("|foo-bar baz", "dw", "|-bar baz")]
#[case("|foo-bar baz", "dW", "|baz")]
#[case("|foo bar baz", "d2w", "|baz")]
#[case("|foo bar baz", "2dw", "|baz")]
#[case("|a b c d e", "2d2w", "|e")]
#[case("|foo\nbar", "d2w", "|")]
#[case("|a\n  b", "d2w", "|")]
#[case("|a b\nc d", "d3w", "|d")]
#[case("foo bar |baz", "db", "foo |baz")]
#[case("foo bar\n|baz", "db", "foo| \nbaz")]
#[case("|foo bar", "de", "| bar")]
#[case("fo|o bar", "d$", "f|o")]
#[case("fo|o bar", "d0", "|o bar")]
#[case("  fo|o", "d^", "  |o")]
#[case("|ab\ncd\nef", "dj", "|ef")]
#[case("ab\n|cd\nef", "dk", "|ef")]
#[case("ab\ncd\n|ef", "dk", "|ab")]
#[case("ab\ncd\n|ef", "dj", "ab\ncd\n|ef")]
#[case("|a\nb\nc\nd", "2dj", "|d")]
#[case("|a\nb\nc\nd", "d2j", "|d")]
#[case("a|b\ncd\nef", "dG", "|")]
#[case("ab\nc|d\nef", "dgg", "e|f")]
// dd x
#[case("ab\nc|d\nef", "dd", "ab\ne|f")]
#[case("ab\ncd\ne|f", "dd", "ab\nc|d")]
#[case("|ab\ncd\nef", "2dd", "|ef")]
#[case("ab\ncd\n|ef", "3dd", "ab\ncd\n|ef")]
#[case("ab\n|cd\nef", "5dd", "|ab")]
#[case("|only", "dd", "|")]
#[case("he|llo", "x", "he|lo")]
#[case("hell|o", "x", "hel|l")]
#[case("he|llo", "3x", "h|e")]
#[case("he|llo", "9x", "h|e")]
#[case("|", "x", "|")]
#[case("a|b\n\ncd", "jx", "ab\n|\ncd")]
#[case("a|ño", "x", "a|o")]
// c
#[case("hel|lo world", "cwX<Esc>", "hel|X world")]
#[case("|foo  bar", "cwX", "X|  bar")]
#[case("foo|  bar", "cwX", "fooX|bar")]
#[case("|a b", "c2wX<Esc>", "|X")]
#[case("|a\nb", "cwX<Esc>", "|X\nb")]
#[case("|ab cd", "cwX<Esc>", "|X cd")]
#[case("a|b cd", "cwX<Esc>", "a|X cd")]
#[case("fo|o bar", "c$X<Esc>", "fo|X")]
#[case("ab\nc|d\nef", "ccX<Esc>", "ab\n|X\nef")]
#[case("ab\nc|d\nef", "cjX<Esc>", "ab\n|X")]
#[case("|ab\ncd", "cc", "|\ncd")]
#[case("|  abc", "cc", "|")]
// y p P
#[case("he|llo world", "yw", "he|llo world")]
#[case("he|llo world", "yw$p", "hello worldllo| ")]
#[case("hello |world", "yb", "|hello world")]
#[case("|abc", "yl3p", "aaa|abc")]
#[case("a|b\ncd", "yyp", "ab\n|ab\ncd")]
#[case("a|b\ncd", "yyP", "|ab\nab\ncd")]
#[case("ab\nc|d", "yyp", "ab\ncd\n|cd")]
#[case("|ab\ncd", "yjGp", "ab\ncd\n|ab\ncd")]
#[case("ab\n|cd", "yk", "|ab\ncd")]
#[case("ab\nc|d", "yk", "a|b\ncd")]
#[case("|ab", "2yyp", "|ab")]
#[case("|  ab\ncd", "yyjp", "  ab\ncd\n  |ab")]
#[case("he|llo", "xp", "hel|lo")]
#[case("|ab\ncd", "ddp", "cd\n|ab")]
#[case("|abc", "x2P", "a|abc")]
#[case("a|b", "p", "a|b")]
#[case("foo b|ar baz", "yiwP", "foo ba|rbar baz")]
// u ctrl-r
#[case("hel|lo world", "dwu", "hel|lo world")]
#[case("foo |bar", "dbu", "|foo bar")]
#[case("foo b|ar", "diwu", "foo |bar")]
#[case("ab\nc|d", "ddu", "ab\nc|d")]
#[case("ab\ncd\ne|f", "ddu", "ab\ncd\ne|f")]
#[case("|abc", "xu<C-r>", "|bc")]
#[case("ab|c", "xu<C-r>", "a|b")]
#[case("|abc", "xxxuu", "|bc")]
#[case("|abc", "xxx3u", "|abc")]
#[case("|abc", "xx2u<C-r>", "|bc")]
#[case("|abc", "u", "|abc")]
#[case("|abc", "<C-r>", "|abc")]
#[case("|abc", "ihello <Esc>u", "|abc")]
#[case("a|bc", "A xyz<Esc>u", "ab|c")]
#[case("a|bc\ndef", "ox<Esc>u", "a|bc\ndef")]
#[case("a|bc\ndef", "Ox<Esc>u", "a|bc\ndef")]
#[case("hel|lo world", "ciwX<Esc>u", "|hello world")]
#[case("hello |world", "cwfoo<Esc>u", "hello |world")]
#[case("ab|c", "ix<Esc>ay<Esc>u", "abx|c")]
#[case("ab|c", "ix<Esc>ay<Esc>uu", "ab|c")]
// i a I A o O
#[case("hel|lo", "iX<Esc>", "hel|Xlo")]
#[case("hel|lo", "iX", "helX|lo")]
#[case("hel|lo", "aX", "hellX|o")]
#[case("hel|lo", "aX<Esc>", "hell|Xo")]
#[case("  hel|lo", "IX<Esc>", "  |Xhello")]
#[case("hel|lo", "AX<Esc>", "hello|X")]
#[case("hel|lo", "A", "hello|")]
#[case("|", "aX<Esc>", "|X")]
#[case("a|b\ncd", "oX<Esc>", "ab\n|X\ncd")]
#[case("a|b\ncd", "OX<Esc>", "|X\nab\ncd")]
#[case("a|b", "o", "ab\n|")]
#[case("a|b", "O", "|\nab")]
#[case("|ab", "ix<CR>y<Esc>", "x\n|yab")]
#[case("ab\n|cd", "i<BS><Esc>", "a|bcd")]
#[case("a|bc", "aX<BS><BS>", "a|c")]
#[case("|abc", "i<BS>", "|abc")]
#[case("he|llo", "ifoo<Esc>", "hefo|ollo")]
#[case("|abc", "<Esc>", "|abc")]
#[case("|abc", "i<Esc>", "|abc")]
#[case("a|bc", "i<Esc>", "|abc")]
// counts y cancelación
#[case("|abc", "d<Esc>x", "|bc")]
#[case("|abc", "3<Esc>x", "|bc")]
#[case("|abc", "dQx", "|bc")]
// iw aw iW aW
#[case("foo b|ar baz", "diw", "foo | baz")]
#[case("foo b|ar baz", "daw", "foo |baz")]
#[case("foo |bar", "daw", "fo|o")]
#[case("foo ba|r  baz", "diw", "foo |  baz")]
#[case("foo  |  bar", "diw", "foo|bar")]
#[case("foo  |  bar", "daw", "fo|o")]
#[case("|foo.bar", "diw", "|.bar")]
#[case("foo.|bar baz", "diW", "| baz")]
#[case("foo.|bar baz", "daW", "|baz")]
#[case("|a b c d", "d2aw", "|c d")]
#[case("|a b c d", "d3iw", "| c d")]
#[case("ab|c", "ciwX<Esc>", "|X")]
#[case("|", "diw", "|")]
#[case("a\n|\nb", "diw", "a\n|\nb")]
// i( a(
#[case("f(a|b) c", "di(", "f(|) c")]
#[case("f(a|b) c", "da(", "f| c")]
#[case("f(a, (|b), c)", "di(", "f(a, (|), c)")]
#[case("f(a, (|b), c)", "d2i(", "f(|)")]
#[case("f(a, (b), |c)", "di)", "f(|)")]
#[case("f(a|b) c", "dib", "f(|) c")]
#[case("f|(ab) c", "di(", "f(|) c")]
#[case("f(ab|) c", "di(", "f(|) c")]
#[case("|foo (bar) baz", "di(", "foo (|) baz")]
#[case("|foo bar", "di(", "|foo bar")]
#[case("foo(\n    b|ar\n)", "di(", "foo(\n|)")]
#[case("f(a \\) |b)", "di(", "f(|)")]
#[case("f|()", "di(", "f(|)")]
#[case("f|()", "ci(X", "f(X|)")]
#[case("f(a|b)", "ci(X<Esc>", "f(|X)")]
// i" a"
#[case("x \"a|b\" y", "di\"", "x \"|\" y")]
#[case("x \"a|b\" y", "da\"", "x |y")]
#[case("x \"a|b\"", "da\"", "|x")]
#[case("|x \"a\" y \"b\"", "di\"", "x \"|\" y \"b\"")]
#[case("x \"a\" |y \"b\"", "di\"", "x \"a\"|\"b\"")]
#[case("x \"a|\" y \"b\"", "di\"", "x \"|\" y \"b\"")]
#[case("x \"a\\\"|b\" y", "di\"", "x \"|\" y")]
#[case("x \"ab|\" y", "ci\"Z<Esc>", "x \"|Z\" y")]
#[case("|foo", "di\"", "|foo")]
#[case("x \"a|b\" y", "d2i\"", "x | y")]
fn cases(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {}

#[apply(cases)]
fn engine(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).text, after);
}

#[apply(cases)]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_oracle(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let nvim = support::nvim::run(before, keys);
    assert_eq!(nvim.text, after, "la tabla no coincide con nvim");
    assert_eq!(nvim.mode, support::run_engine(before, keys).mode);
}

/// nvim no tiene multi-cursor: estos casos solo prueban el motor.
#[rstest]
#[case("a|b a|b", "x", "a| |a")]
#[case("|foo |bar", "dw", "|")]
#[case("|foo\n|bar", "A!<Esc>", "foo|!\nbar|!")]
#[case("a|b\nc|d", "iX<Esc>", "a|Xb\nc|Xd")]
#[case("|ab\n|cd", "o", "ab\n|\ncd\n|")]
#[case("|ab\n|cd", "ddu", "|ab\n|cd")]
#[case("|ab\n|cd", "yyp", "ab\n|ab\ncd\ncd\n|ab\ncd")]
#[case("f|oo b|ar", "ciwX<Esc>", "|X |X")]
fn multiple_cursors(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).text, after);
}

#[test]
fn reports_what_each_key_did() {
    use nota_vim::{Action, InsertAt, Key, KeyOutcome, Motion, Vim};

    let (mut buffer, mut selections) = support::parse("|ab");
    let mut vim = Vim::new().unwrap();
    let mut feed = |key| vim.feed(key, &mut buffer, &mut selections).unwrap();
    assert_eq!(feed(Key::Char('g')), KeyOutcome::Pending);
    assert_eq!(
        feed(Key::Char('g')),
        KeyOutcome::Executed(Action::Motion(Motion::DocumentStart))
    );
    assert_eq!(feed(Key::Char('3')), KeyOutcome::Pending);
    assert_eq!(feed(Key::Char('Z')), KeyOutcome::Unbound);
    assert_eq!(
        feed(Key::Char('i')),
        KeyOutcome::Executed(Action::Insert(InsertAt::Before))
    );
    assert_eq!(feed(Key::Char('x')), KeyOutcome::Inserted);
}

#[test]
fn actions_run_by_name() {
    use nota_vim::{Action, Vim};

    let (mut buffer, mut selections) = support::parse("|ab");
    let mut vim = Vim::new().unwrap();
    let delete_right = Action::from_name("vim::delete_right").unwrap();
    vim.execute(delete_right, &mut buffer, &mut selections)
        .unwrap();
    assert_eq!(support::render(&buffer, [selections.primary().head]), "|b");
}

#[test]
fn operators_wait_in_their_own_mode() {
    use nota_vim::{Key, Mode, Operator, Vim};

    let (mut buffer, mut selections) = support::parse("|ab");
    let mut vim = Vim::new().unwrap();
    vim.feed(Key::Char('d'), &mut buffer, &mut selections)
        .unwrap();
    assert_eq!(vim.mode(), Mode::OperatorPending(Operator::Delete));
    vim.feed(Key::Escape, &mut buffer, &mut selections).unwrap();
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(buffer.to_string(), "ab");
}
