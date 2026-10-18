#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Búsqueda (`/`, `?`, `n`, `N`, `*`, `#`) y `:s`. Cada caso de la tabla
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
// lo negado y [:space:] no cruzan el fin de línea
#[case("|xa\nab", "/a[^a]<CR>", "xa\n|ab")]
#[case("|xa\na b", "/a[[:space:]]<CR>", "xa\n|a b")]
#[case("|xa\nab", r"/a\S<CR>", "xa\n|ab")]
#[case("|xa\na b", r"/a\W<CR>", "xa\n|a b")]
#[case("|xa\nab", r"/a\D<CR>", "xa\n|ab")]
#[case("|xa\nb", "/a[^a]<CR>", "|xa\nb")]
#[case("a b\nxa\n|c", "?a[^a]<CR>", "|a b\nxa\nc")]
#[case("|xa\nab", ":%s/a[^a]/Y/<CR>", "xa\n|Y")]
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
// . tras d* y d# vuelve a leer la palabra bajo el cursor, y n la busca
#[case("|aa x aa bb x bb", "d*w.", "aa |bb")]
#[case("|aa x aa bb x bb", "d*wn", "|aa bb x bb")]
#[case("|aa x aa bb x bb y bb", "d*w.n", "aa bb y |bb")]
#[case("aa x a|a b", "d#.", "|a b")]
#[case("q aa x a|a aa", "d#u.", "q |aa")]
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
#[case("|a b c", "/c<CR>:s/a/X/<CR>0/<Up><CR>", "|X b c")]
#[case("|ab\nab", ":s/a/X/<CR>j:<Up><CR>", "Xb\n|Xb")]
// :s, :%s, rangos y flags
#[case("|a,a,a\na,a", ":s/a/b/<CR>", "|b,a,a\na,a")]
#[case("|a,a,a\na,a", ":s/a/b/g<CR>", "|b,b,b\na,a")]
#[case("|a,a,a\na,a", ":%s/a/b/g<CR>", "b,b,b\n|b,b")]
#[case("  a,a,a\n  a,|a\nx", ":%s/a/b/g<CR>", "  b,b,b\n  |b,b\nx")]
#[case("a\nb\n|c\nd\ne", ":2,4s/$/!/<CR>", "a\nb!\nc!\n|d!\ne")]
#[case("a\nb\n|c\nd\ne", ":.,$s/^/-/<CR>", "a\nb\n-c\n-d\n|-e")]
#[case("a\nb\n|c\nd\ne", ":.-1,.+1s/^/-/<CR>", "a\n-b\n-c\n|-d\ne")]
#[case("|ab\nab", ":%s/ab/x<CR>", "x\n|x")]
#[case("|ab", ":s/b<CR>", "|a")]
#[case("|ab", ":5s/a/x/<CR>", "|ab")]
#[case("|Abc abc", ":s/a/X/g<CR>", "|Xbc Xbc")]
#[case("|Abc abc", ":s/A/X/g<CR>", "|Xbc abc")]
#[case("|Abc abc", ":s/a/X/gI<CR>", "|Abc Xbc")]
#[case("|Abc abc", ":s/A/X/gi<CR>", "|Xbc Xbc")]
// i e I valen solo para ese :s: lo que se recuerda vuelve a smartcase
#[case("|Foo foo foo", ":s/Foo/X/i<CR>n", "|X foo foo")]
#[case("|Foo foo foo", ":s/Foo/X/i<CR>/<CR>", "|X foo foo")]
#[case("|Foo foo\nFoo foo", ":s/Foo/X/gi<CR>j:s<CR>", "X X\n|X foo")]
#[case("|Foo foo\nFoo foo", ":s/Foo/X/gi<CR>j:s//Y/g<CR>", "X X\n|Y foo")]
#[case("|abc\nabc", ":s/z/X/<CR>", "|abc\nabc")]
#[case("|abc\nabc", ":s/z/X/e<CR>", "|abc\nabc")]
#[case("|a/b", ":s#/#-#<CR>", "|a-b")]
#[case(r"|a/b", r":s/\//-/<CR>", "|a-b")]
#[case("|añb", ":s/ñ/n/<CR>", "|anb")]
#[case("|a.b", ":s/./x/g<CR>", "|xxx")]
#[case("|a b", r":s/\s/_/<CR>", "|a_b")]
// el reemplazo: grupos, &, coincidencias vacías
#[case(
    "|foo bar",
    r":s/\(foo\) \(bar\)/\2 \1 & \&/<CR>",
    "|bar foo foo bar &"
)]
#[case("|abc", ":s/b/[&]/<CR>", "|a[b]c")]
#[case("|abc", ":s/x*/-/g<CR>", "|-a-b-c")]
#[case("|baab", ":s/a*/-/g<CR>", "|-b-b")]
#[case("|baab", r":s/a\=/-/g<CR>", "|-b--b")]
// :%s que cambia el número de líneas, con \r
#[case("|a,b,c\nd,e", r":%s/,/\r/g<CR>", "a\nb\nc\nd\n|e")]
#[case("|a,b\nc,d\ne", r":1s/,/\r/<CR>", "a\n|b\nc,d\ne")]
#[case("|abc", r":s/c/1\r2\r3/<CR>", "ab1\n2\n|3")]
// :s sin patrón: el último usado, de búsqueda o de :s
#[case("|abc\nabc", "/b<CR>:s//X/<CR>", "|aXc\nabc")]
#[case("|abc\nabc", ":s/b/X/<CR>jn", "aXc\na|bc")]
#[case("|abc\nabc", ":s/b/X/<CR>j:s<CR>", "aXc\n|aXc")]
#[case("|abc\nabc", ":s/b/X/<CR>/c<CR>j:s<CR>", "aXc\n|aXc")]
#[case("|abc", ":s//X/<CR>", "|abc")]
#[case("|abc", ":s<CR>", "|abc")]
// todo :s es un solo paso de undo, y el undo vuelve al inicio de la primera línea cambiada
#[case("|ab ab", ":s/ab/x/g<CR>u", "|ab ab")]
#[case("  a,a,a\n  a,|a\nx", ":%s/a/b/g<CR>u", "|  a,a,a\n  a,a\nx")]
#[case("  x\n  a,|a\nx\n  a", ":%s/a/b/g<CR>u", "  x\n|  a,a\nx\n  a")]
#[case("  x\n  a,|a\nx\n  a", ":%s/a/b/g<CR>u<C-r>", "  x\n|  b,b\nx\n  b")]
#[case("|a,b,c\nd,e", r":%s/,/\r/g<CR>u", "|a,b,c\nd,e")]
#[case("|ab", ":s/a/x/<CR>:s/b/y/<CR>u", "|xb")]
#[case("|a\na", ":s/a/b/<CR>j.", "b\n|a")]
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
