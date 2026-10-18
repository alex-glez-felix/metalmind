#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Búsqueda (`/`, `?`, `n`, `N`, `*`, `#`). Cada caso de la tabla
//! corre en el motor y en nvim, con `ignorecase` y `smartcase` como el motor.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};

/// Cada caso es `(antes, teclas, después)`, como en `vim.rs`. El `<` va como
/// `<lt>`, porque nvim lo leería como el inicio de una tecla. No hay casos a
/// medio escribir la búsqueda: nvim mueve el cursor con `incsearch` y el motor
/// no.
#[template]
#[rstest]
// / ? con <CR>, y wrap en las dos direcciones
#[case("a|bc foo bar foo", "/foo<CR>", "abc |foo bar foo")]
#[case("a|bc foo bar foo", "/foo<CR>n", "abc foo bar |foo")]
#[case("abc foo bar |foo", "/foo<CR>", "abc |foo bar foo")]
#[case("|foo", "/foo<CR>", "|foo")]
#[case("abc\n|def\nabc", "?abc<CR>", "|abc\ndef\nabc")]
#[case("abc\n|def\nabc", "?abc<CR>n", "abc\ndef\n|abc")]
#[case("|abc\nxabc", "?abc<CR>", "abc\nx|abc")]
#[case("|a b a b", "2/b<CR>", "a b a |b")]
#[case("|a b a b", "/b<CR>2n", "a |b a b")]
#[case("|a b c b", "/b/<CR>", "a |b c b")]
// el patrón no está, búsqueda vacía
#[case("a|bc\ndef", "/zz<CR>", "a|bc\ndef")]
#[case("|abc", "/zz<CR>x", "|bc")]
#[case("|a b c", "/<CR>", "|a b c")]
#[case("|a b c b", "/b<CR>0/<CR>", "a |b c b")]
#[case("|a b c b", "/b<CR>0?<CR>", "a b c |b")]
#[case("|a b c b", "/b<CR>0?<CR>n", "a |b c b")]
#[case("|a x", "n", "|a x")]
#[case("|a x", "N", "|a x")]
// editar y cancelar la línea de búsqueda
#[case("|abc", "/b<BS><BS>x", "|bc")]
#[case("|abc", "/x<BS>c<CR>", "ab|c")]
#[case("|abc", "/b<Esc>x", "|bc")]
#[case("|abc", "/b<C-c>x", "|bc")]
// n y N después de / y ?
#[case("|a x b x c x", "/x<CR>N", "a x b x c |x")]
#[case("a x b x c |x", "?x<CR>n", "a |x b x c x")]
#[case("a x b x c |x", "?x<CR>N", "a x b x c |x")]
#[case("abc\n|def\nabc", "?abc<CR>N", "abc\ndef\n|abc")]
// . * [] ^ $ y el resto del dialecto
#[case("|xa1b a.b", "/a.b<CR>", "x|a1b a.b")]
#[case("|xa1b a.b", r"/a\.b<CR>", "xa1b |a.b")]
#[case("|ab aab", "/a*b<CR>", "ab |aab")]
#[case("|x*a", "/*a<CR>", "x|*a")]
#[case("|x a1 b2", "/[0-9]<CR>", "x a|1 b2")]
#[case("|x a1 b2", "/[^ a-z]<CR>", "x a|1 b2")]
#[case("|x]a", "/[]]<CR>", "x|]a")]
#[case("|x[a", "/[a<CR>", "x|[a")]
#[case("|xab\nab", "/^ab<CR>", "xab\n|ab")]
#[case("|ab x\nab", "/ab$<CR>", "ab x\n|ab")]
#[case("|a$b", "/$b<CR>", "a|$b")]
#[case("|xa^b", "/a^<CR>", "x|a^b")]
#[case("|foobar foo", r"/\<lt>foo\><CR>", "foobar |foo")]
#[case("|foo xfoo", r"/\<lt>foo<CR>", "|foo xfoo")]
#[case("|ab abab", r"/\(ab\)\+<CR>", "ab |abab")]
#[case("|ac abc", r"/ab\=c<CR>", "ac |abc")]
#[case("|a1 b22", r"/\d\{2}<CR>", "a1 b|22")]
#[case("|x a+b", "/a+b<CR>", "x |a+b")]
#[case("|a/b c", r"/\/<CR>", "a|/b c")]
#[case("|a?b c", r"?\?<CR>", "a|?b c")]
#[case("|x ab cd", r"/cd\|ab<CR>", "x |ab cd")]
// ignorecase + smartcase, \c y \C
#[case("|Foo foo", "/foo<CR>", "Foo |foo")]
#[case("|foo Foo", "/Foo<CR>", "foo |Foo")]
#[case("|foo Foo FOO", r"/\Cfoo<CR>", "|foo Foo FOO")]
#[case("|foo Foo", r"/F\co<CR>", "foo |Foo")]
// multibyte
#[case("|x añob ñob", "/ñob<CR>", "x a|ñob ñob")]
#[case("|x añob ñob", "/ñob<CR>n", "x añob |ñob")]
#[case("aé|b é", "?é<CR>", "a|éb é")]
#[case("|🦀a 🦀b", "/🦀b<CR>", "🦀a |🦀b")]
#[case("|ñ", "/.<CR>", "|ñ")]
#[case("|ÁRBOL árbol", "/árbol<CR>", "ÁRBOL |árbol")]
// $ y ^ en los bordes de la línea
#[case("ab|c\ndef", "/$<CR>", "abc\nde|f")]
#[case("ab|c\ndef", "/$<CR>n", "ab|c\ndef")]
#[case("ab|c\ndef", "?$<CR>", "abc\nde|f")]
#[case("ab|c\ndef", "?^<CR>", "|abc\ndef")]
#[case("|abc\ndef", "/^<CR>", "abc\n|def")]
#[case("abc\n|\ndef", "/^$<CR>", "abc\n|\ndef")]
#[case("|baab", "/a*<CR>", "b|aab")]
#[case("b|aab", "/a*<CR>", "baa|b")]
// * # g* g#
#[case("|foo foobar foo", "*", "foo foobar |foo")]
#[case("|foo foobar foo", "g*", "foo |foobar foo")]
#[case("foo foobar |foo", "#", "|foo foobar foo")]
#[case("foo foobar |foo", "g#", "foo |foobar foo")]
#[case("fo|o x foo", "*", "foo x |foo")]
#[case("fo|o x foo", "#", "foo x |foo")]
#[case("x foo y fo|o", "#", "x |foo y foo")]
#[case("|Foo foo", "*", "Foo |foo")]
#[case("|Foo foo FOO", "*n", "Foo foo |FOO")]
#[case("|foo foo foo foo", "2*", "foo foo |foo foo")]
#[case("|foo x foo", "*N", "|foo x foo")]
#[case("|a.b a.b axb", "*", "a.b |a.b axb")]
#[case("|  foo x foo", "*", "  foo x |foo")]
#[case("|añob x añob", "*", "añob x |añob")]
#[case("x\n|\nfoo", "*", "x\n|\nfoo")]
// * sobre puntuación: busca la palabra que sigue, y si no hay, la puntuación
#[case("a |... b ... c", "*", "a ... |b ... c")]
#[case("a b |...", "*", "a b |...")]
#[case("x |.. ..", "*", "x .. |..")]
#[case("a b |... x ...", "g*", "a b ... |x ...")]
#[case("|x.* x.*", "W#", "|x.* x.*")]
// como motion de un operador
#[case("|foo bar baz", "d/baz<CR>", "|baz")]
#[case("foo bar |baz", "d?bar<CR>", "foo |baz")]
#[case("foo bar |baz", "c?bar<CR>X<Esc>", "foo |Xbaz")]
#[case("|foo bar baz", "c/baz<CR>X<Esc>", "|Xbaz")]
#[case("|a x b x c x", "/x<CR>0dn", "|x b x c x")]
#[case("|a x b x c x", "/x<CR>0dn.", "|x c x")]
#[case("|a b x c x", "/x<CR>0dN", "|x")]
#[case("|a x b x c x", "d/x<CR>.", "|x c x")]
#[case("|a x b x c x d", "d/x<CR>/c<CR>.n", "|x b x x d")]
#[case("|a x\nb x", "d/b<CR>", "|b x")]
#[case("  a|b\n  cd", "d/cd<CR>", "  a|cd")]
#[case("a b |c a", "d/b<CR>", "a |c a")]
#[case("|abc", "d/zz<CR>", "|abc")]
#[case("|abc", "d/zz<CR>x", "|bc")]
#[case("|abc", "d/b<Esc>x", "|bc")]
#[case("|a b c b", "d2/b<CR>", "|b")]
#[case("|a b c b", "2d/b<CR>", "|b")]
#[case("fo|o x foo", "d*", "fo|foo")]
#[case("fo|o x foo", "d#", "fo|foo")]
#[case("|a x b x", "dn", "|a x b x")]
#[case("|ab ab", "d/b<CR>u", "|ab ab")]
// historial con las flechas
#[case("|a b c b c", "/b<CR>/c<CR>0/<Up><Up><CR>", "a |b c b c")]
#[case("|a b c b c bx", "/bx<CR>/c<CR>/b<CR>0/b<Up><CR>", "a |b c b c bx")]
#[case("|a b c b c bx", "/bx<CR>/c<CR>/b<CR>0/b<Up><Up><CR>", "a b c b c |bx")]
#[case("|a b c b c bx", "/bx<CR>/c<CR>0/<Up><Up><Down><CR>", "a b |c b c bx")]
#[case("|a b c b c bx", "/bx<CR>/c<CR>0/z<Up><Down><CR>", "|a b c b c bx")]
#[case("|a b c b c bx", "/b<CR>/c<CR>/b<CR>0/<Up><Up><CR>", "a b |c b c bx")]
#[case("|a b c b c", "/b<CR>/c<CR>0/<Up>x<BS><Up><CR>", "a b |c b c")]
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
