#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Visual por carácter (`v`) y por línea (`V`). Como en `vim.rs`, cada caso
//! corre en el motor y en `nvim --clean`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};

/// `(antes, teclas, después)` para los casos que terminan fuera de Visual.
#[template]
#[rstest]
// entrar, salir y operar
#[case("a|bcd", "vld", "a|d")]
#[case("a|bc\ndef", "vlld", "a|def")]
#[case("abc|d\nab", "vjd", "ab|c")]
#[case("abc|d\nab", "vj<Esc>", "abcd\na|b")]
#[case("ab|c", "v$<Esc>x", "a|b")]
#[case("ab|c", "vl<Esc>x", "a|b")]
#[case("a|bc", "vl<C-c>x", "a|b")]
#[case("|abc", "vv", "|abc")]
#[case("|abc", "vvx", "|bc")]
#[case("|abc", "VV", "|abc")]
#[case("|abcdef", "vllo<Esc>x", "|bcdef")]
#[case("|abcdef", "vllohd", "|def")]
#[case("a|bc\nde\nfg", "vjVd", "f|g")]
#[case("a|bc\nde\nfg", "Vjvd", "|a\nfg")]
#[case("a|bc\nde\nfg", "vjvx", "abc\n|d\nfg")]
#[case("a|bc\nde\nfg", "Vjoj<Esc>x", "abc\n|d\nfg")]
#[case("a|bc\nde\nfg", "Vjokd", "f|g")]
// hacia atrás
#[case("ab|cd", "vhhd", "|d")]
#[case("ab|cd", "vhhy", "|abcd")]
#[case("a\nab|cd\nef\ngh", "Vky", "|a\nabcd\nef\ngh")]
#[case("a\nab|cd\nef\ngh", "Vkd", "e|f\ngh")]
#[case("a,b,c,|d", "vF,,d", "a,b,|c")]
#[case("ab\nc|d", "vkd", "|a")]
#[case("ab\nc|d", "vkoh<Esc>x", "ab\n|d")]
// línea vacía y fin de archivo
#[case("ab|c\n\nfg", "vjd", "ab|fg")]
#[case("ab|c\n\nfg", "vj<Esc>x", "abc\n|\nfg")]
#[case("ab\n|\ncd", "vd", "ab\n|cd")]
#[case("ab\n|\ncd", "Vd", "ab\n|cd")]
#[case("ab\n|", "vd", "ab\n|")]
#[case("ab\n|", "Vd", "|ab")]
#[case("a|b c", "vwd", "|a")]
#[case("a|b c", "vey", "a|b c")]
#[case("a\nb|c\nde", "vGd", "a\n|b")]
#[case("a\nb|c\nde", "vggd", "|a\nde")]
#[case("a\nb|c\nde", "Vggd", "d|e")]
#[case("ab\nc|d", "Vd", "a|b")]
#[case("|ab", "Vd", "|")]
#[case("ab|c", "vlld", "a|b")]
#[case("a|bc\nd", "v$$d", "a|d")]
// counts
#[case("a|b\nc\nd\ne", "3Vd", "|e")]
#[case("a|b\nc\nd\ne", "9Vd", "|")]
#[case("|abcdef", "3vd", "|def")]
#[case("|abcdef", "9vd", "|")]
#[case("|abcdef", "vlyl2vd", "a|f")]
#[case("|ab\ncd\nef\ngh", "vjy2vd", "|h")]
#[case("ab|cd\nef", "Vy2Vd", "|")]
#[case("a|b\ncd\nef\ngh", "Vjy2Vd", "|")]
#[case("|abcdef", "v3ld", "|ef")]
#[case("a|b\ncd\nef\ngh", "V2jd", "g|h")]
#[case("|abc\nd", "v$y3vd", "|d")]
#[case("|ab\ncd\nef", "Vjyj2vd", "|ab")]
// motions
#[case("a,|b,c,d", "vf,d", "a,|c,d")]
#[case("a,|b,c,d", "vt,;d", "a,|,d")]
#[case("a\nb|c\nd", "v2Gd", "a\n|b\nd")]
#[case("a|bc\ndef\ngh", "vj$d", "a|gh")]
#[case("a|bc\nd\nghij", "v$jjd", "|a")]
#[case("foo |bar", "vbd", "|ar")]
#[case("|foo bar baz", "v2ed", "| baz")]
#[case("|foo bar", "vWd", "|ar")]
#[case("foo.ba|r x", "vBd", "| x")]
#[case("  a|b", "v^d", " | ")]
#[case("  a|b", "v0d", "|")]
#[case("lo|ng\nab\nlong", "vjjd", "lo|g")]
// búsqueda
#[case("a|bc foo bar", "v/foo<CR>d", "a|oo bar")]
#[case("abc foo |bar", "v?abc<CR>d", "|ar")]
#[case("|a b a b", "v/b<CR>nd", "|")]
#[case("|a b a b", "v2/b<CR>d", "|")]
#[case("|a b a b", "/b<CR>0vnd", "| a b")]
#[case("|a b a b", "/b<CR>0vNd", "|")]
#[case("a|bc foo", "v/zz<CR>d", "a|c foo")]
#[case("a|bc foo", "vl/foo<Esc>d", "a| foo")]
#[case("a|bc foo", "vl/<BS>d", "a| foo")]
#[case("a|bc\nfoo\nx", "V/foo<CR>d", "|x")]
#[case("a|bc foo bar foo", "v/foo<CR>d.", "a|ar foo")]
#[case("abc f|oo", "v/$<CR>d", "abc |f")]
#[case("a|bc\nfoo", "v/foo<CR>y", "a|bc\nfoo")]
// registros, marks y macros
#[case("|ab cd", "v\"ayw\"ap", "ab c|ad")]
#[case("|ab cd", "wv\"ay0v\"ap", "|cb cd")]
#[case("|ab cd", "yiwwviwp0\"-p", "ac|db ab")]
#[case("|ab cd", "wv\"_d0p", "|ab d")]
#[case("|a\nb\nc", "Vjd\"1p", "c\n|a\nb")]
#[case("|ab cd ab", "v/cd<CR>\"ay$\"ap", "ab cd abab |c")]
#[case("|ab cd ef", "v\"adw.\"ap", "b d|c ef")]
#[case("ab|c def", "ma0v`ad", "| def")]
#[case("|a\nb\nc", "jmajV'ad", "|a")]
#[case("|ab cd ef", "qavlUwq@a", "AB CD |ef")]
#[case("|ab cd ef", "qavlUwqu@a", "AB |cd ef")]
// multibyte
#[case("añ|ob c", "vld", "añ| c")]
#[case("a|ñob c", "vlld", "a| c")]
#[case("|ñandú x", "ved", "| x")]
#[case("a|ñ\nb", "vjd", "|a")]
#[case("ñ|ñ\nb", "v$d", "ñ|b")]
#[case("x |é y", "viwd", "x | y")]
// text objects de palabra
#[case("foo b|ar baz qux", "viwd", "foo | baz qux")]
#[case("foo b|ar baz qux", "viwiwd", "foo |baz qux")]
#[case("foo b|ar baz qux", "viwiwiwd", "foo | qux")]
#[case("foo b|ar baz qux", "vawd", "foo |baz qux")]
#[case("foo b|ar baz qux", "vawawd", "foo |qux")]
#[case("foo b|ar baz qux", "vhiwd", "foo|r baz qux")]
#[case("foo b|ar baz qux", "vhhhhawd", "|r baz qux")]
#[case("foo b|ar baz", "v2iwd", "foo |baz")]
#[case("foo b|ar baz", "Viwd", "foo | baz")]
#[case("a.b c|.d e", "viWd", "a.b | e")]
#[case("a.b c|.d e", "vaWd", "a.b |e")]
#[case("foo bar b|az", "vhhhhhiwd", "foo|z")]
#[case("foo bar b|az", "vbiwiwd", "foo |z")]
#[case("foo b|ar", "vawawd", "fo|o")]
#[case("foo   |  bar", "viwd", "foo|bar")]
#[case("a\n|b c", "vkiwd", "| c")]
// paréntesis
#[case("f(a, (b|c), d)", "vi(d", "f(a, (|), d)")]
#[case("f(a, (b|c), d)", "vi(i(d", "f(|)")]
#[case("f(a, (b|c), d)", "va(d", "f(a, |, d)")]
#[case("f(a, (b|c), d)", "va(a(d", "|f")]
#[case("f(a, (b|c), d)", "vi(a(d", "f(a, |, d)")]
#[case("f(a, (b|c), d)", "va(i(d", "f(|)")]
#[case("f(|)", "vi(d", "f|(")]
#[case("(a(|)b)", "vi(d", "(a(|b)")]
#[case("f(a, (b|c), d)", "vhi(d", "f(|)")]
#[case("f(a, (b|c), d)", "vli(d", "f(a, (|), d)")]
#[case("f(a, (b|c), d)", "v2i(d", "f(|)")]
#[case("x |a (b) c", "vi(d", "x a (|) c")]
#[case("f(\n  a|b\n)", "vi(d", "f(\n|)")]
#[case("f(\n  a|b\n  )", "vi(d", "f(\n|  )")]
#[case("f(a|b)", "Vi(d", "f(|)")]
#[case("(a) |b", "vi(d", "(a)| ")]
// comillas
#[case("x \"a|b\" y \"cd\" z", "vi\"d", "x \"|\" y \"cd\" z")]
#[case("x \"a|b\" y \"cd\" z", "vi\"i\"d", "x | y \"cd\" z")]
#[case("x \"a|b\" y \"cd\" z", "vi\"i\"i\"d", "x |\" z")]
#[case("x \"a|b\" y \"cd\" z", "va\"d", "x |y \"cd\" z")]
#[case("x \"a|b\" y \"cd\" z", "va\"a\"d", "x |z")]
#[case("x \"a|b\" y \"cd\" z", "vlli\"d", "x \"a|\" z")]
#[case("x \"a|b\" y \"cd\" z", "vhi\"d", "x | y \"cd\" z")]
#[case("x \"ab\" y \"c|d\" z", "vha\"d", "x \"ab\" y |z")]
#[case("x \"|\" y", "vi\"d", "x | y")]
#[case("x |\"ab\" y", "vi\"d", "x \"|\" y")]
#[case("x \"a|b\" y", "v2i\"d", "x | y")]
#[case("x \"a|b\"\nz", "vji\"d", "x \"|a")]
#[case("x \"a|b\" y \"cd\" z", "vllllllli\"d", "x \"a|\" z")]
#[case("x \"ab\" y \"c|d\" z", "vhhhhhhhi\"d", "x \"|\" z")]
#[case("\"añ|o\" x", "vi\"d", "\"|\" x")]
// párrafos
#[case("a\nb|b\ncc\n\nd\ne\n\nf", "vipd", "|\nd\ne\n\nf")]
#[case("a\nb|b\ncc\n\nd\ne\n\nf", "vipipd", "|d\ne\n\nf")]
#[case("a\nb|b\ncc\n\nd\ne\n\nf", "vipipipd", "|\nf")]
#[case("a\nb|b\ncc\n\nd\ne\n\nf", "vapapd", "|f")]
#[case("a\nb|b\ncc\n\nd\ne\n\nf", "vjjipd", "a\n|b\n\nf")]
#[case("a\nb|b\ncc\n\nd\ne\n\nf", "Vkipd", "|cc\n\nd\ne\n\nf")]
#[case("a\nbb\ncc\n\nd\ne\n\n|f", "vkkkipd", "a\nbb\ncc\n|")]
#[case("|\n\nx", "Vipipd", "|")]
#[case("a\nb|b\n\nc", "Vipd", "|\nc")]
#[case("|a\nb\n\nc", "Vipd", "|\nc")]
#[case("|a\nb\n\nc\nd", "Vipipd", "|c\nd")]
#[case("a\nb\n\n|c\nd", "vkkapd", "|\nd")]
#[case("a\nb|b\ncc\n\nd", "v2ipd", "|d")]
#[case("a\nb|b\ncc\n\nd", "vapd", "|d")]
// c
#[case("ab|c de", "vecX<Esc>", "ab|X")]
#[case("a\nb|b\ncc\nd", "VjcX<Esc>", "a\n|X\nd")]
#[case("ab|c\nde", "v$cX<Esc>", "ab|Xde")]
#[case("a|bc\nde", "vjcX<Esc>", "a|X")]
#[case("|ab", "VcX<Esc>", "|X")]
#[case("a|bc", "vlcX<Esc>u", "a|bc")]
// y p P
#[case("ab|c\nd", "v$yjP", "abc\n|c\nd")]
#[case("ab|c\nd", "v$y", "ab|c\nd")]
#[case("|abc def", "yiwwvp", "abc ab|cef")]
#[case("|abc def", "yiwwvpu", "abc |def")]
#[case("|abc def", "yiwwvp0P", "|dabc abcef")]
#[case("|abc def", "yiwwvP0P", "ab|cabc abcef")]
#[case("|ab\ncd", "yyjvp", "ab\n\n|ab\nd")]
#[case("|ab\ncd", "yyjlvp", "ab\nc\n|ab\n")]
#[case("|ab\ncd", "yiwjVp", "ab\n|ab")]
#[case("|ab\ncd\nef", "yyjVp", "ab\n|ab\nef")]
#[case("|ab\ncd\nef", "yyjVjp", "ab\n|ab")]
#[case("|ab\ncd", "yyVjp", "|ab")]
#[case("|ab cd", "yiwwv2p", "ab aba|bd")]
#[case("|ab\ncd\nef", "yyjV2p", "ab\n|ab\nab\nef")]
#[case("|abc\nd", "ywjvp", "abc\nab|c")]
#[case("|b\nacd", "v$ojyjlvp", "b\n|a\nad")]
#[case("|ab\ncd", "yjjvp", "ab\n\n|ab\ncd\nd")]
#[case("|ab\ncd xy", "vjyjwvp", "ab\ncd |ab\ncy")]
#[case("ab |cd", "yyvP", "ab \n|ab cd\nd")]
#[case("|abc", "vlp", "|c")]
#[case("|abc", "vlP", "|c")]
#[case("|ab\ncd", "yiwjV2p", "ab\n|ab\nab")]
#[case("|ab\ncd", "Vyjvp", "ab\n\n|ab\nd")]
#[case("a|b\ncd", "Vjyp", "ab\n|ab\ncd\ncd")]
// r
#[case("a|bcd\nef", "vjrx", "a|xxx\nxx")]
#[case("a|bcd\nef", "vlrñ", "a|ññd\nef")]
#[case("añ|bcd", "vlrx", "añ|xxd")]
#[case("ab|cd", "vhr<Esc>d", "a|d")]
#[case("a|bcd\nef", "Vjrx", "|xxxx\nxx")]
#[case("a|bc", "vlr<Tab>", "a|\t\t")]
#[case("a|b\n\ncd", "vjjrx", "a|x\n\nxx")]
// mayúsculas
#[case("a|bC dÉ", "v$~", "a|Bc Dé")]
#[case("a|bC dÉ", "v$u", "a|bc dé")]
#[case("a|bC dÉ", "v$U", "a|BC DÉ")]
#[case("a|bc\nde", "vjU", "a|BC\nDE")]
#[case("|aB\ncD", "Vj~", "|Ab\nCd")]
#[case("a|bc", "vlUu", "a|bc")]
#[case("|aBcD", "g~l", "|ABcD")]
#[case("|aBcD", "g~~", "|AbCd")]
#[case("|aB cD", "gUw", "|AB cD")]
#[case("a|B cD", "guu", "|ab cd")]
#[case("a|B cD", "gUiw", "|AB cD")]
#[case("  a|B\n cD", "gUU", "  |AB\n cD")]
#[case("  a|B\n cD", "gU2U", "  A|B\n CD")]
#[case("a|B\ncD", "g~j", "A|b\nCd")]
#[case("  a|B cD", "g~wu", "  a|B cD")]
#[case("|ab cd", "gUw.w.", "AB |CD")]
#[case("|ab\ncd", "gUU", "|AB\ncd")]
#[case("|ab\ncd", "2gUU", "|AB\nCD")]
#[case("a|b", "g~<Esc>x", "|a")]
// .
#[case("|abc def", "yiwwvep0l.", "a|abc")]
#[case("|abc def ghi", "yiwwvePw.", "abc abc| ")]
#[case("a|bcdef", "vlrxll.", "axx|xxf")]
#[case("a|bc\ndef\nghi", "vjrxj.", "axx\nx|xx\nxxi")]
#[case("a|bcdef", "vl~ll.", "aBC|DEf")]
#[case("ab|cdef", "vlcX<Esc>ll.", "abXe|X")]
#[case("a|b\ncd\nef\ngh", "VjcX<Esc>j.", "X\n|X")]
#[case("|abc\ndef\nghi\njkl", "vjdj.", "ef\n|kl")]
#[case("|abc\ndef\nghi\njkl", "vjd3.", "|hi\njkl")]
#[case("|abcdef", "vld3.", "|ef")]
#[case("a|b\ncd\nef\ngh", "Vjdj.", "e|f")]
#[case("a|b\ncd\nef\ngh", "Vjyj.", "ab\n|cd\nef\ngh")]
#[case("|abc\ndef\nghi", "v$dj.", "def\n|")]
#[case("|ab cd ef", "vlx.", "|d ef")]
#[case("|abc\ndef\nghi", "vjdu.", "|ef\nghi")]
#[case("|abcdef", "vldu.", "|cdef")]
// gv
#[case("a|bcdef", "vll<Esc>0gvd", "a|ef")]
#[case("a|bcdef", "vlly$gvd", "a|ef")]
#[case("a|b\ncd\nef", "Vj<Esc>Ggvd", "e|f")]
#[case("a|bcdef", "vll<Esc>gvo<Esc>x", "a|cdef")]
#[case("|abcdef", "vl<Esc>$vgvd", "|cdef")]
#[case("|abcdef", "vl<Esc>$vgvgvd", "abcd|e")]
#[case("|abcdef", "gvd", "|abcdef")]
#[case("ab|c\nd", "v$<Esc>0gvd", "ab|d")]
#[case("|abcdef", "vlldgvd", "|")]
#[case("|ab\ncd", "Vjygvd", "|")]
// undo, la columna de `$` al entrar y `gv` tras operar
#[case("a|bc\nde", "vjrxu", "a|bc\nde")]
#[case("a|bc", "vl~u", "a|bc")]
#[case("ab|c\nde", "$vjd", "a|b")]
#[case("ab|c\nde\nfgh", "$vjdj.", "a|b")]
#[case("|ab cd", "veyw2vd", "ab| ")]
#[case("a|b\ncd", "Vjrx.", "|xx\nxx")]
#[case("|abc", "vlUgvd", "|c")]
// columnas de pantalla con tabs, también en `.` y `[count]v`
#[case("a\tb|c\nxyzwvutsrqp", "vjd", "a\tb|p")]
#[case("a\tb|c\nxyzwvutsrqp", "vkd", "a\t|b\nxyzwvutsrqp")]
#[case("xyzwvut|srqp\na\tbc", "vjd", "xyzwvut|bc")]
#[case("xyzwvutsr|qp\na\tbc", "vjohd", "xyzwvut|s")]
#[case("x\t|ab\nxyz\tcd", "vlyj0vd", "x\tab\n|yz\tcd")]
#[case("|a\tbc\na\tbc", "vlly1vd", "|c\na\tbc")]
#[case("|a\tbc\na\tbc\nxyzwvutsrqp", "vlldj0.", "c\n|c\nxyzwvutsrqp")]
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

/// Los que terminan en Visual: `después` marca también el otro extremo de
/// la selección con `¦`.
#[template]
#[rstest]
#[case("|abc", "v", "|abc")]
#[case("|abc", "vl", "¦a|bc")]
#[case("ab|c", "vh", "a|b¦c")]
#[case("ab|c", "vho", "a¦b|c")]
#[case("ab|c", "vhol", "a¦bc|")]
#[case("|abc", "V", "|abc")]
#[case("|ab\ncd", "Vj", "¦ab\n|cd")]
#[case("|ab\ncd", "vjV", "¦ab\n|cd")]
#[case("a|bc\nde", "vjo", "a|bc\nd¦e")]
#[case("a|bc\nde", "vj$", "a¦bc\nde|")]
#[case("a|bc", "v$", "a¦bc|")]
#[case("a|bc", "vlll", "a¦bc|")]
#[case("ab|c\nab", "vj", "ab¦c\nab|")]
#[case("abc|d\nab", "vj", "abc¦d\nab|")]
#[case("a|b\n\ncd", "vj", "a¦b\n|\ncd")]
#[case("|abc", "3v", "¦ab|c")]
#[case("|ab\ncd\nef", "3V", "¦ab\ncd\n|ef")]
#[case("|abc", "vly3v", "¦abc|")]
#[case("|abc", "vlyl1v", "a¦b|c")]
#[case("foo b|ar baz", "viw", "foo ¦ba|r baz")]
#[case("foo b|ar baz", "viwiw", "foo ¦bar| baz")]
#[case("foo b|ar baz", "vaw", "foo ¦bar| baz")]
#[case("foo b|ar baz", "vhiw", "foo| b¦ar baz")]
#[case("foo b|ar baz", "vhaw", "|foo b¦ar baz")]
#[case("foo b|ar baz", "Viw", "foo ¦ba|r baz")]
#[case("foo |bar", "vbiw", "|foo ¦bar")]
#[case("a b|c d", "vlaw", "a b¦c |d")]
#[case("f(a, (b|c), d)", "vi(", "f(a, (¦b|c), d)")]
#[case("f(a, (b|c), d)", "va(", "f(a, ¦(bc|), d)")]
#[case("f(a, (b|c), d)", "vi(i(", "f(¦a, (bc), |d)")]
#[case("f(\n  a|b\n)", "vi(", "f(\n¦  ab|\n)")]
#[case("f(|)", "vi(", "f(|)")]
#[case("x \"a|b\" y", "vi\"", "x \"¦a|b\" y")]
#[case("x \"a|b\" y", "va\"", "x ¦\"ab\"| y")]
#[case("x \"a|b\" y", "vi\"i\"", "x ¦\"ab|\" y")]
#[case("x \"|\" y", "vi\"", "x |\"¦\" y")]
#[case("x \"a|b\" y \"cd\"", "vhi\"", "x |\"ab¦\" y \"cd\"")]
#[case("a\nb|b\n\nc", "vip", "¦a\n|bb\n\nc")]
#[case("a\nb|b\n\nc", "vipip", "¦a\nbb\n|\nc")]
#[case("a\nb|b\n\nc", "Vip", "¦a\n|bb\n\nc")]
#[case("a\nb\n\n|c", "vkkip", "|a\nb\n\n¦c")]
#[case("x|y", "vlr<Esc>", "x¦y|")]
#[case("|abc", "vf<Esc>", "|abc")]
#[case("|abcdef", "vll<Esc>gv", "¦ab|cdef")]
#[case("|abcdef", "vl<Esc>$vgv", "¦a|bcdef")]
#[case("|ab\ncd", "Vj<Esc>gv", "¦ab\n|cd")]
#[case("|abc", "vl<Esc>gvv", "a|bc")]
#[case("|abc", "vlo", "|a¦bc")]
#[case("a|b\ncd", "Vjo", "a|b\nc¦d")]
#[case("a|bc\ndef", "vjoj", "abc\nd|ef")]
#[case("a|bc foo", "v/foo<CR>", "a¦bc |foo")]
#[case("abc |foo", "v?b<CR>", "a|bc ¦foo")]
#[case("a|bc foo", "v/zz<CR>", "a|bc foo")]
#[case("a|bc foo", "vl/foo<Esc>", "a¦b|c foo")]
#[case("|ab\nfoo", "V/foo<CR>", "¦ab\n|foo")]
#[case("|a b a b", "/b<CR>0vn", "¦a |b a b")]
#[case("ab |cd", "ma0v`a", "¦ab |cd")]
#[case("|ab\ncd", "jmakv'a", "¦ab\n|cd")]
#[case("a b |a b", "/b<CR>vNo", "a ¦b a |b")]
fn selections(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {}

#[apply(selections)]
fn engine_selection(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).selection, after);
}

#[apply(selections)]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_selection(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let nvim = support::nvim::run(before, keys);
    assert_eq!(nvim.selection, after, "la tabla no coincide con nvim");
    assert_eq!(nvim.mode, support::run_engine(before, keys).mode);
}

/// nvim no tiene multi-cursor: estos casos solo prueban el motor.
#[rstest]
#[case("a|b a|b", "vd", "a| |a")]
#[case("|ab\n|cd", "vlU", "|AB\n|CD")]
#[case("|ab\n|cd", "Vd", "|")]
#[case("f|oo b|ar", "viwcX<Esc>", "|X |X")]
#[case("|abc |def", "vlrx", "|xxc |xxf")]
#[case("|ab |cd", "vl<Esc>gvd", "| ")]
#[case("|abcd\n|efgh", "vld.", "|\n|")]
#[case("|ab\n|cd", "vl<Esc>x", "|a\n|c")]
#[case("f|oo b|ar", "viwo<Esc>x", "|oo |ar")]
#[case("|ab x. |ab x.", "v/x<CR>d", "|. |.")]
fn multiple_cursors(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).text, after);
}

#[test]
fn reports_the_visual_mode() {
    use nota_vim::{Key, Mode, Vim, VisualKind};

    let (mut buffer, mut selections) = support::parse("|ab\ncd");
    let mut vim = Vim::new();
    let mut feed = |vim: &mut Vim, key| vim.feed(key, &mut buffer, &mut selections).unwrap();
    feed(&mut vim, Key::Char('v'));
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Char));
    feed(&mut vim, Key::Char('V'));
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Line));
    feed(&mut vim, Key::Escape);
    assert_eq!(vim.mode(), Mode::Normal);
}

#[test]
fn the_selection_keeps_its_anchor() {
    use nota_buffer::ByteOffset;
    use nota_vim::Vim;

    let (mut buffer, mut selections) = support::parse("a|bcd");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "vll");
    let selection = selections.primary();
    assert_eq!(selection.anchor, ByteOffset::new(1));
    assert_eq!(selection.head, ByteOffset::new(3));
}

/// Como un clic en vim: termina la selección y quien aloja el editor pone
/// el cursor.
#[test]
fn moving_the_cursor_ends_visual() {
    use nota_buffer::{ByteOffset, Selection, Selections};
    use nota_vim::{Mode, Vim};

    let (mut buffer, mut selections) = support::parse("|abc def");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "vl");
    selections = Selections::single(Selection::cursor(ByteOffset::new(4)));
    vim.cursor_moved();
    assert_eq!(vim.mode(), Mode::Normal);
    support::feed(&mut vim, &mut buffer, &mut selections, "x");
    let heads = selections.iter().map(|s| s.head);
    assert_eq!(support::render(&buffer, heads), "abc |ef");
}

#[test]
fn visual_actions_run_by_name() {
    use nota_vim::{Action, Vim};

    let (mut buffer, mut selections) = support::parse("|abc");
    let mut vim = Vim::new();
    for name in ["vim::toggle_visual", "vim::right", "vim::uppercase"] {
        let action = Action::from_name(name).unwrap();
        vim.execute(action, &mut buffer, &mut selections).unwrap();
    }
    assert_eq!(
        support::render(&buffer, [selections.primary().head]),
        "|ABc"
    );
}

#[test]
fn replace_waits_for_the_char() {
    use nota_vim::{Action, Key, KeyOutcome, Mode, Vim, VisualKind};

    let (mut buffer, mut selections) = support::parse("|ab");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "v");
    let pending = vim.feed(Key::Char('r'), &mut buffer, &mut selections);
    assert_eq!(pending.unwrap(), KeyOutcome::Executed(Action::Replace));
    let cancelled = vim.feed(Key::Escape, &mut buffer, &mut selections);
    assert_eq!(cancelled.unwrap(), KeyOutcome::Executed(Action::NormalMode));
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Char));
    assert_eq!(buffer.to_string(), "ab");
}
