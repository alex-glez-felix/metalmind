#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
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
// la columna objetivo es de pantalla: un tab llega a la siguiente parada y
// un carácter ancho ocupa dos
#[case("a\tb|c\nxyzwvutsrqp", "j", "a\tbc\nxyzwvutsr|qp")]
#[case("a|\tbc\nxyzwvutsrqp", "j", "a\tbc\nxyzwvut|srqp")]
#[case("xyzwvut|srqp\na\tbc", "j", "xyzwvutsrqp\na|\tbc")]
#[case(
    "xyzwvut|srqp\na\tbc\nxyzwvutsrqp",
    "jj",
    "xyzwvutsrqp\na\tbc\nxyzwvut|srqp"
)]
#[case("xy|zw\na日本\nxyzw", "j", "xyzw\na|日本\nxyzw")]
#[case("xy|zw\na日本\nxyzw", "jj", "xyzw\na日本\nxy|zw")]
#[case("a|日本\nxyzw", "j", "a日本\nx|yzw")]
#[case("a\tb|c\nxyzwvutsrqp", "jx", "a\tbc\nxyzwvutsr|p")]
#[case("\t|ab\n\tcd\nxyzwvutsrqp", "jjx", "\tab\n\tcd\nxyzwvuts|qp")]
// zt, zz y zb con count van a la línea en el mismo byte, como nvim, y
// buscan después la columna de pantalla de antes
#[case("añb|cd\nxyzwvutsrq", "2zt", "añbcd\nxyzw|vutsrq")]
#[case("a\tb|c\nxyzwvutsrqp", "2zz", "a\tbc\nxyz|wvutsrqp")]
#[case("abc|d\nññññ", "2zb", "abcd\nñ|ñññ")]
#[case("abcdefg|h\nab", "2zt", "abcdefgh\na|b")]
#[case("añb|cd\nxyzwvutsrq", "2ztk", "añb|cd\nxyzwvutsrq")]
#[case("añb|cd\nxyzwvutsrq", "l2ztk", "añbc|d\nxyzwvutsrq")]
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
// f F t T
#[case("|a,b,c", "f,", "a|,b,c")]
#[case("|a,b,c", "2f,", "a,b|,c")]
#[case("|a,b,c", "3f,", "|a,b,c")]
#[case("|abc", "fz", "|abc")]
#[case("ab|c", "fc", "ab|c")]
#[case("|a,b\nc,d", "2f,", "|a,b\nc,d")]
#[case("a,b,|c", "F,", "a,b|,c")]
#[case("a,b,|c", "2F,", "a|,b,c")]
#[case("|abc", "Fa", "|abc")]
#[case("|ab,c", "t,", "a|b,c")]
#[case("|a,b,c", "t,", "|a,b,c")]
#[case("|a,b,c", "2t,", "a,|b,c")]
#[case("a,bc|d", "T,", "a,|bcd")]
#[case("a,|bc", "T,", "a,|bc")]
#[case("|a3b", "f3", "a|3b")]
#[case("|a b", "f ", "a| b")]
#[case("|a\tb", "f<Tab>", "a|\tb")]
#[case("|abcd", "f<Esc>x", "|bcd")]
#[case("|abcd", "3f<Esc>x", "|bcd")]
#[case("|aéb🦀c", "fé", "a|éb🦀c")]
#[case("|aéb🦀c", "f🦀", "aéb|🦀c")]
#[case("|aéb🦀c", "t🦀", "aé|b🦀c")]
#[case("aéb🦀|c", "Fé", "a|éb🦀c")]
#[case("aéb🦀|c", "Té", "aé|b🦀c")]
#[case("|ñañaña", "2fñ", "ñaña|ña")]
// ; ,
#[case("|a,b,c,d", "f,;", "a,b|,c,d")]
#[case("|a,b,c,d", "f,;;", "a,b,c|,d")]
#[case("|a,b,c,d", "f,;,", "a|,b,c,d")]
#[case("|a,b,c,d", "f,2;", "a,b,c|,d")]
#[case("|a,b,c,d", "f,9;", "a|,b,c,d")]
#[case("a,b,c,|d", "F,;", "a,b|,c,d")]
#[case("a,b,c,|d", "F,;,", "a,b,c|,d")]
#[case("|a)b)c)", "t);", "a)|b)c)")]
#[case("|a)b)c)", "t);;", "a)b)|c)")]
#[case("|a)b)c)", "t)2;", "a)|b)c)")]
#[case("|a)b)c)", "t);;,", "a)|b)c)")]
#[case("(a(b(|c", "T(;", "(a(|b(c")]
#[case("(a(b(|c", "T(,", "(a(b(|c")]
#[case("|aéb🦀cé", "té;", "aéb🦀|cé")]
#[case("|abc", ";", "|abc")]
#[case("|abc", ",", "|abc")]
#[case("|a,b,c", "f,fz;", "a|,b,c")]
// d c y con f t F T ; ,
#[case("|a,b,c", "df,", "|b,c")]
#[case("|a,b,c", "dt,", "|,b,c")]
#[case("a|bc", "dtc", "a|c")]
#[case("|a,b,c", "d2f,", "|c")]
#[case("|a,b,c", "2df,", "|c")]
#[case("|a,b,c", "d3f,", "|a,b,c")]
#[case("|a,b,c", "dfz", "|a,b,c")]
#[case("a,b,|c", "dF,", "a,b|c")]
#[case("a,b,|c", "dT,", "a,b,|c")]
#[case("a,bc|d", "dT,", "a,|d")]
#[case("|aéb🦀c", "df🦀", "|c")]
#[case("aéb🦀|c", "dTé", "aé|c")]
#[case("|a,b,c,d", "f,d;", "a|c,d")]
#[case("|a,b,c,d", "f,d2;", "a|d")]
#[case("|a,b,c,d", "t,d;", "|,c,d")]
#[case("a,b,c,|d", "T,d,", "a,b,c,|d")]
#[case("|a,b,c", "df,;", "b|,c")]
#[case("|a,b,c", "d;", "|a,b,c")]
#[case("|f(a, b)", "ct)X<Esc>", "|X)")]
#[case("f(a|, b)", "ct)X<Esc>", "f(a|X)")]
#[case("|a,b,c", "cfzX<Esc>", "|a,b,c")]
#[case("|a,b,c", "yf,P", "a|,a,b,c")]
#[case("call(x)|;", "yT(P", "call(x|)x);")]
#[case("call(x)|;", "yF(P", "call(x|)(x);")]
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
#[case("|b\nac", "v$ojyp", "|b\na\nac")]
#[case("|ab\ncd", "lv$ojyjp", "ab\nc|d\ncd")]
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
// g- g+
#[case("|abc", "g-", "|abc")]
#[case("|abc", "g+", "|abc")]
#[case("|abc", "xxg-", "|bc")]
#[case("|abc", "xxg-g+", "|c")]
#[case("|abc", "xu$xg-", "|bc")]
#[case("|abc", "xu$xg-g-", "|abc")]
#[case("|abc", "xu$xg-g-g+", "|bc")]
#[case("|abc", "xu$xg-g-g+g+", "a|b")]
#[case("|abc", "xu$x2g-", "ab|c")]
#[case("|abc", "xu$x2g-2g+", "a|b")]
#[case("|abc", "xu$x9g-9g+", "a|b")]
#[case("|abc", "xu$xg+", "a|b")]
#[case("|abc", "xu$xg-u<C-r>", "|bc")]
#[case("ab|c", "xu0xg-", "a|b")]
#[case("ab|c", "xu0xg-u<C-r><C-r>", "a|b")]
#[case("|abc", "xxuuxg-g-g-", "|abc")]
#[case("|abc", "ix<Esc>uiy<Esc>g-", "|xabc")]
#[case("|abc", "ix<Esc>uiy<Esc>g-g+", "|yabc")]
#[case("ab\nc|d\nef", "ddukddg-", "ab\ne|f")]
#[case("ab\nc|d\nef", "ddukddg-g-", "ab\nc|d\nef")]
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
// count en Insert y ctrl-c, que sale sin repetir
#[case("a|bc", "3ihola<Esc>", "aholaholahol|abc")]
#[case("a|bc", "3ihola<C-c>", "ahol|abc")]
#[case("a|bc", "3ihola", "ahola|bc")]
#[case("a|bc", "ihola<C-c>", "ahol|abc")]
#[case("a|bc", "3ihola<Esc>u", "a|bc")]
#[case("a|bc", "3ihola<C-c>u", "a|bc")]
#[case("a|bc", "3ix<Esc>u<C-r>", "a|xxxbc")]
#[case("a|bc", "aX<C-c>", "ab|Xc")]
#[case("a|bc", "3aX<Esc>", "abXX|Xc")]
#[case("a|bc", "3aX<C-c>", "ab|Xc")]
#[case("a|bc", "3IX<Esc>", "XX|Xabc")]
#[case("a|bc", "3AX<Esc>", "abcXX|X")]
#[case("a|bc\ndef", "3oX<Esc>", "abc\nX\nX\n|X\ndef")]
#[case("a|bc\ndef", "3OX<Esc>", "X\nX\n|X\nabc\ndef")]
#[case("a|bc\ndef", "3oX<C-c>", "abc\n|X\ndef")]
#[case("a|bc", "2o<Esc>", "abc\n\n|")]
#[case("a|bc", "3iab<BS>c<Esc>", "aacaca|cbc")]
#[case("a|bc", "3ia<CR>b<Esc>", "aa\nba\nba\n|bbc")]
#[case("a|bc", "2i<BS><Esc>", "|bc")]
#[case("a|bc", "2i<BS>x<Esc>", "|xbc")]
#[case("a|bc", "3i<Esc>", "|abc")]
#[case("a|bc d e", "2cwX<Esc>", "a|X e")]
#[case("ab|c", "ix<C-c>ay<Esc>u", "abx|c")]
#[case("|abc", "ix<C-c><C-c>x", "|abc")]
#[case("|abc", "<C-c>x", "|bc")]
#[case("|abc", "d<C-c>x", "|bc")]
#[case("ab|c", ":w<C-c>x", "a|b")]
// lo pegado en Insert entra como lo tecleado
#[case("|abc", "3ia<PasteStart>XY<PasteEnd><Esc>", "aXYaXYaX|Yabc")]
#[case("|abc", "3ia<PasteStart>XY<PasteEnd>z<Esc>", "aXYzaXYzaXY|zabc")]
#[case("|abc", "3ia<PasteStart>XY<PasteEnd><C-c>", "aX|Yabc")]
#[case("|abc", "3i<PasteStart>X\nY<PasteEnd><Esc>", "X\nYX\nYX\n|Yabc")]
#[case("|abc", "ia<PasteStart>XY<PasteEnd><Esc>u", "|abc")]
#[case("ab|c", ":<PasteStart>w<PasteEnd><Esc>x", "a|b")]
// un clic en Insert descarta la repetición del count
#[case(
    "|abc def",
    "3ihola<LeftMouse><10,0><LeftRelease><10,0><Esc>",
    "holaabc d|ef"
)]
#[case(
    "|abc def",
    "3ihola<LeftMouse><2,0><LeftRelease><2,0><Esc>",
    "h|olaabc def"
)]
#[case(
    "|abc def",
    "3ihola<LeftMouse><10,0><LeftRelease><10,0>x<Esc>",
    "holaabc de|xf"
)]
#[case("|abc", "3ix<LeftMouse><20,0><LeftRelease><20,0>y<Esc>", "xabc|y")]
#[case(
    "|abc\ndef",
    "3ix<LeftMouse><1,1><LeftRelease><1,1>y<Esc>",
    "xabc\nd|yef"
)]
#[case(
    "|abc",
    "3ix<LeftMouse><2,0><LeftRelease><2,0><PasteStart>YZ<PasteEnd><Esc>",
    "xaY|Zbc"
)]
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
// iw aw iW aW con un count que no alcanza: fallan y el cursor queda donde
// llegó buscando
#[case("foo |qux", "d2aw", "foo qu|x")]
#[case("foo |qux bar", "d3aw", "foo qux ba|r")]
#[case("foo |qux", "d3iw", "foo qu|x")]
#[case("foo |qux bar", "d4iw", "foo qux ba|r")]
#[case("foo |q.x", "d2aW", "foo q.|x")]
#[case("foo |q.x b.r", "d3aW", "foo q.x b.|r")]
#[case("foo |q.x", "d3iW", "foo q.|x")]
#[case("foo |qux\nbar", "d2aw", "fo|o")]
#[case("foo |qux\nbar", "d3aw", "foo qux\nba|r")]
#[case("foo |qux\nbar", "d5iw", "foo qux\nba|r")]
#[case("foo |qux\n\nbar", "d3aw", "foo qux\n\nba|r")]
#[case("foo |qux\n", "d3aw", "foo qux\n|")]
#[case("f|oo bar", "d9aw", "foo ba|r")]
#[case("foo |qux  ", "d2aw", "foo qux | ")]
#[case("foo |qux  ", "d4iw", "foo qux | ")]
#[case("foo |qux  \nbar", "d4aw", "foo qux  \nba|r")]
#[case("foo |qux", "c2aw", "foo qu|x")]
#[case("foo |qux bar", "c3aw", "foo qux ba|r")]
#[case("foo |qux", "c3iw", "foo qu|x")]
#[case("foo |qux", "y2aw", "foo qu|x")]
#[case("foo |qux bar", "y3aw", "foo qux ba|r")]
#[case("foo |qux", "yiwy2awP", "foo ququ|xx")]
#[case("foo b|ar baz qux", "2daw.", "foo qu|x")]
#[case("abc fo|o\nx\nlonger text", "$d2a(jj", "abc foo\nx\nlonger| text")]
#[case("a|bc\nxy\nlonger", "$dfzjj", "abc\nxy\nlonge|r")]
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
// ip ap
#[case("a\n|b\n\nc\nd\n\ne", "yap", "|a\nb\n\nc\nd\n\ne")]
#[case("abcd\nab|cd\n\n   wxyz", "yip", "|abcd\nabcd\n\n   wxyz")]
#[case("abcd\nab|cd\n\n   wxyz", "dip", "|\n   wxyz")]
#[case("abcd\nab|cd\n\n   wxyz", "dap", "  | wxyz")]
#[case("a\n|b\n\nc\nd\n\ne", "dip", "|\nc\nd\n\ne")]
#[case("a\n|b\n\nc\nd\n\ne", "dap", "|c\nd\n\ne")]
#[case("a\nb\n\n|c\nd\n\ne", "dap", "a\nb\n\n|e")]
#[case("a\nb\n\nc\nd\n\n|e", "dap", "a\nb\n\nc\n|d")]
#[case("a\nb\n\nc\nd\n\n|e", "dip", "a\nb\n\nc\nd\n|")]
#[case("x\n\nabcd\nab|cd", "dap", "|x")]
#[case("x\n\nabcd\nab|cd", "yap", "x\n|\nabcd\nabcd")]
#[case("x\n\nabcd\nab|cd", "dip", "x\n|")]
#[case("|a\nb", "dap", "|")]
#[case("|a\nb", "cipX<Esc>", "|X")]
#[case("|a\n\n\nb", "cap", "|\nb")]
#[case("a\nb\n|\nc", "dap", "a\n|b")]
#[case("|a\n\nb\n\nc", "d2ip", "|b\n\nc")]
#[case("|a\n\nb\n\nc", "2dip", "|b\n\nc")]
#[case("|a\n\nb\n\nc", "d2ap", "|c")]
#[case("|a\n\nb\n\nc", "d9ip", "|a\n\nb\n\nc")]
#[case("|a\n\nb\n\nc", "d9ap", "|a\n\nb\n\nc")]
#[case("a\n|\nb\n\nc", "dip", "a\n|b\n\nc")]
#[case("a\n|\nb\n\nc", "dap", "a\n|\nc")]
#[case("a\n|\nb\n\nc", "d2ip", "a\n|\nc")]
#[case("a\n|  \nb", "dip", "a\n|b")]
#[case("a\n\n|\n\nb", "dip", "a\n|b")]
#[case("\n|\n\n", "dip", "|")]
#[case("\n|\n\n", "dap", "\n|\n\n")]
#[case("\n|\n\n", "yip", "|\n\n\n")]
#[case("|", "dip", "|")]
#[case("|", "dap", "|")]
#[case(
    "al|fa beta\ngama\n\ndelta",
    "yipP",
    "|alfa beta\ngama\nalfa beta\ngama\n\ndelta"
)]
// . y su count: `3.` reemplaza el del cambio y queda para los siguientes
#[case("|abcdef", ".", "|abcdef")]
#[case("|abcdef", "3.", "|abcdef")]
#[case("|abcdef", "x.", "|cdef")]
#[case("|abcdef", "2x.", "|ef")]
#[case("|abcdef", "2x3.", "|f")]
#[case("|abcdef", "x3.", "|ef")]
#[case("|a b c d e f g h", "dw.", "|c d e f g h")]
#[case("|a b c d e f g h", "2dw.", "|e f g h")]
#[case("|a b c d e f g h", "d2w.", "|e f g h")]
#[case("|a b c d e f g h", "2dw..", "|g h")]
#[case("|a b c d e f g h", "2dw3.", "|f g h")]
#[case("|a b c d e f g h", "2dw3..", "|")]
#[case("|a b c d e f g h", "d3w2.", "|f g h")]
#[case("|a b c d e f g h i j k l m n", "2d3w.", "|m n")]
#[case("|a b c d e f g h", "2d3w2.", "|")]
#[case("|ab\ncd\nef\ngh", "dd.", "|ef\ngh")]
#[case("|ab\ncd\nef\ngh", "2dd.", "|")]
#[case("|a\n\nb\n\nc\n\nd", "dap.", "|c\n\nd")]
#[case("|a\n\nb\n\nc\n\nd", "dip.", "|b\n\nc\n\nd")]
#[case("foo b|ar baz qux", "diw.", "foo|baz qux")]
#[case("foo b|ar baz qux", "daw.", "foo |qux")]
#[case("foo b|ar baz qux quux zap", "2daw.", "foo |zap")]
#[case("f(a|b) g(cd)", "di(fc.", "f() g(|)")]
// . al final del archivo y con un count mayor que lo que queda
#[case("ab|c", "x.", "|a")]
#[case("|abc", "x9.", "|")]
#[case("|a b", "dw9.", "|")]
#[case("ab\n|cd", "dd.", "|")]
#[case("ab\ncd\n|ef", "dd.", "|ab")]
#[case("|ab\ncd\nef", "dd5.", "|")]
#[case("|ab\ncd", "dd..", "|")]
// . repite la pasada por Insert entera, aunque se salga con ctrl-c
#[case("a|bc", "ihola<Esc>.", "aholhol|aabc")]
#[case("a|bc", "ihola<Esc>3.", "aholholaholahol|aabc")]
#[case("a|bc", "3ihola<Esc>.", "aholaholaholholaholahol|aabc")]
#[case("a|bc", "3ihola<Esc>2.", "aholaholaholholahol|aabc")]
#[case("a|bc", "ihola<C-c>.", "aholhol|aabc")]
#[case("a|bc", "3ihola<C-c>.", "aholholaholahol|aabc")]
#[case("a|bc\ndef", "A;<Esc>j.", "abc;\ndef|;")]
#[case("a|bc\ndef", "oX<Esc>.", "abc\nX\n|X\ndef")]
#[case("a|bc\ndef", "2oX<Esc>.", "abc\nX\nX\nX\n|X\ndef")]
#[case("a|bc\ndef", "OX<Esc>.", "|X\nX\nabc\ndef")]
#[case("|ab\ncd", "o<Esc>.", "ab\n\n|\ncd")]
#[case("ab|cd", "xi<Esc>.", "|abd")]
#[case("|abc", "ix<CR>y<Esc>.", "x\nx\n|yyabc")]
#[case("foo |bar baz", "ciwX<Esc>w.", "foo X |X")]
#[case("foo |bar baz", "cwX<Esc>w.", "foo X |X")]
#[case("|a b c d", "2cwX<Esc>w.", "X |X")]
#[case("|ab cd", "cwX<C-c>w.", "X |X")]
#[case("|ab\ncd\nef", "ccX<Esc>j.", "X\n|X\nef")]
#[case("x \"a|b\" \"cd\"", "ci\"Z<Esc>fc.", "x \"Z\" \"|Z\"")]
#[case("|abc def", "ia<PasteStart>XY<PasteEnd>z<Esc>0.", "aXY|zaXYzabc def")]
#[case("|abc", "3ia<PasteStart>XY<PasteEnd><Esc>0.", "aXYaXYaX|YaXYaXYaXYabc")]
// . tras un <BS>: repite las teclas, no el texto que quedó
#[case("a|bc", "iab<BS>c<Esc>.", "aaa|ccbc")]
#[case("a|bc", "i<BS><Esc>.", "|bc")]
#[case("ab\n|cd", "i<BS><Esc>.", "|bcd")]
#[case("|abcd", "3iab<BS>c<Esc>.", "acacaacaca|ccabcd")]
// . con multibyte
#[case("|añob c", "x.", "|ob c")]
#[case("|ñandú", "x..", "|dú")]
#[case("|ñ🦀é x", "dw.", "|é x")]
#[case("a|bc", "ié<Esc>.", "a|éébc")]
#[case("a|bc", "ié🦀<BS>ñ<Esc>.", "aéé|ññbc")]
// . con f t F T: repite la búsqueda tecleada, y tras d; la de ; en ese momento
#[case("|a,b,c,d", "df,.", "|c,d")]
#[case("|a,b,c,d", "dt,.", "|,c,d")]
#[case("a,b,c,|d", "dF,.", "a,b|d")]
#[case("a,b,c,|d", "dT,.", "a,b,c,|d")]
#[case("|a,b,c,d", "dt,;.", ",|,c,d")]
#[case("|a,b,c,d", "df,;.", "b|d")]
#[case("|a,b;c;d", "df,f;.", "b|;c;d")]
#[case("|a,b,c,d", "f,d;.", "a|d")]
#[case("|a,b,c;d;e", "f,d;f;.", "ac|e")]
#[case("|a,b;c,d;e,f", "dt,f;.;", ",b,d|;e,f")]
#[case("|a,b;c,d;e,f", "dt,f;.,", ",b|,d;e,f")]
// un . que falla deja el cambio de antes, con su count, y no ejecuta el resto
#[case("|a,b,c,d,e", "df,5.", "|b,c,d,e")]
#[case("|a,b,c,d,e", "df,5..", "|c,d,e")]
#[case("|a,b c", "ct,Y<Esc>$.", "Y,b |c")]
#[case("|a,b c", "ct,Y<Esc>$.u", "|a,b c")]
#[case("|abcabc", "xdfz.", "|cabc")]
#[case("|abcabc", "xd;.", "|cabc")]
// . después de p y P: pega lo que haya en el registro al repetir
#[case("|ab", "ylp.", "aa|ab")]
#[case("|ab", "yl2p.", "aaaa|ab")]
#[case("|ab", "yl2p3.", "aaaaa|ab")]
#[case("|ab\ncd", "yyp.", "ab\nab\n|ab\ncd")]
#[case("|ab\ncd", "yyP.", "|ab\nab\nab\ncd")]
#[case("|ab cd", "xp.", "ba|a cd")]
#[case("|ab cd", "xywp.", "bb b|  cd")]
#[case("|abc", "ix<Esc>pyl.", "x|xabc")]
// lo que no se graba: motions, yank, undo, la línea de comandos
#[case("|abc def", "xw.", "bc |ef")]
#[case("|abc def", "xwyl.", "bc |ef")]
#[case("|ab cd", "xyw.", "| cd")]
#[case("|abc def", "xu.", "|bc def")]
#[case("|abc", "xu<C-r>.", "|c")]
#[case("|abc def", "xg-.", "|bc def")]
#[case("|abc", "ix<Esc>u.", "|xabc")]
#[case("|abc", "x:<Esc>.", "|c")]
#[case("|abc", "ix<Esc>:w<Esc>.", "|xxabc")]
#[case("ab|c", "f<Esc>.", "ab|c")]
#[case("|abc", "xd..", "|c")]
#[case("|abcdef", "xd.x", "|cdef")]
// un cambio que no borra nada también se graba
#[case("|ab cd ef\n\nxy", "dwjxk.", "|d ef\n\nxy")]
#[case("|abc\n\nef", "xjdiwk.", "|\n\nef")]
// . es un solo paso de undo
#[case("|abcdef", "x3.u", "|bcdef")]
#[case("|a b c d", "dw.u", "|b c d")]
#[case("a|bc", "ihola<Esc>3.u", "ahol|abc")]
#[case("|ab\ncd", "yyp.u", "ab\n|ab\ncd")]
#[case("|ab\ncd", "yyp.uu", "|ab\ncd")]
#[case("foo |bar baz", "ciwX<Esc>w.u", "foo X |baz")]
#[case("a|bc\ndef", "oX<Esc>.u", "abc\n|X\ndef")]
#[case("|ab cd", "cwX<Esc>u.", "|X cd")]
// tras un clic en Insert, lo que se teclee es otro cambio para .: un `i` ahí
#[case(
    "|abc def",
    "3ihola<LeftMouse><6,0><LeftRelease><6,0>x<Esc>0.",
    "|xholaabxc def"
)]
#[case(
    "|abc def",
    "ihola<LeftMouse><6,0><LeftRelease><6,0><Esc>0.",
    "hol|aholaabc def"
)]
#[case(
    "|abc def",
    "3ihola<LeftMouse><6,0><LeftRelease><6,0><Esc>0.",
    "holaholahol|aholaabc def"
)]
#[case(
    "|abc def",
    "Ahola<LeftMouse><2,0><LeftRelease><2,0>x<Esc>0.",
    "|xabxc defhola"
)]
#[case(
    "|abc def",
    "oX<LeftMouse><0,0><LeftRelease><0,0>y<Esc>j.",
    "yabc def\n|yX"
)]
#[case(
    "|abc def",
    "cwX<LeftMouse><4,0><LeftRelease><4,0>y<Esc>0.",
    "|yX deyf"
)]
#[case("|abc def", "cwX<LeftMouse><4,0><LeftRelease><4,0><Esc>w.", "X de|X")]
#[case(
    "|abc def",
    "ix<LeftMouse><4,0><LeftRelease><4,0><BS><Esc>$.",
    "xab |df"
)]
#[case(
    "|abc def",
    "ix<LeftMouse><4,0><LeftRelease><4,0><PasteStart>Q<PasteEnd><Esc>0.",
    "|QxabcQ def"
)]
// registros con nombre "a–"z: guardan lo copiado o borrado y p respeta si es por líneas
#[case("foo |bar baz", "\"ayiw$\"ap", "foo bar bazba|r")]
#[case("foo |bar baz", "\"ayiwyiw$\"ap", "foo bar bazba|r")]
#[case("a|b\ncd", "\"ayyj\"ap", "ab\ncd\n|ab")]
#[case("a|b\ncd", "\"ayyj\"aP", "ab\n|ab\ncd")]
#[case("|ab\ncd", "\"addj\"ap", "cd\n|ab")]
#[case("|ab\ncd", "\"ayyjyy\"ap", "ab\ncd\n|ab")]
#[case("a|b\ncd", "\"ayyp", "ab\n|ab\ncd")]
#[case("|abc", "\"ax\"bx\"ap\"bp", "ca|b")]
#[case("|abc", "\"bxp", "b|ac")]
#[case("foo |bar", "\"acwX<Esc>$\"ap", "foo Xba|r")]
// con mayúscula se agrega al final, y uno por líneas vuelve todo por líneas
#[case("foo |bar baz", "\"ayiww\"Ayiw$\"ap", "foo bar bazbarba|z")]
#[case("|foo bar", "\"ayw\"Ayy\"ap", "foo bar\n|foo \nfoo bar")]
#[case("|foo\nbar", "\"ayyj\"Ayw\"ap", "foo\nbar\n|foo\nbar")]
#[case("|foo\nbar", "\"ayyj\"Ayy\"ap", "foo\nbar\n|foo\nbar")]
#[case("|foo\nbar", "\"Ayy\"ap", "foo\n|foo\nbar")]
#[case("|ab cd", "\"Ax\"ap", "b|a cd")]
#[case("|ab\ncd", "\"ayy\"Addp", "cd\n|ab\nab")]
// count antes y después del registro
#[case("|abcdef", "2\"a3x\"ap", "abcde|f")]
#[case("|abc", "\"a3yl$\"ap", "abcab|c")]
#[case("|abc", "yl\"a2p", "|abc")]
#[case("a|b\ncd", "\"ayy3\"ap", "ab\n|ab\nab\nab\ncd")]
// "0 guarda el último yank sin registro
#[case("|ab cd", "ywwdw\"0p", "ab ab| ")]
#[case("|ab cd", "yw\"byww\"0p", "ab cab| d")]
#[case("|ab\ncd", "yyjdd\"0p", "ab\n|ab")]
#[case("|ab\ncd", "yyj\"_dd\"0p", "ab\n|ab")]
#[case("|ab\ncd", "yyj\"_ddp", "ab\n|ab")]
#[case("|ab\ncd\nef", "ddyy\"_ddp", "ef\n|cd")]
#[case("|ab\ncd", "\"\"yyj\"0p", "ab\ncd\n|ab")]
#[case("|ab\ncd\nef", "yyj\"\"dd\"0p", "ab\nef\n|cd")]
// "1–"9: los borrados de líneas se recorren
#[case("|a\nb\nc\nd", "dddddd\"1p\"2p\"3p", "d\nc\nb\n|a")]
#[case("|a\nb\nc\nd", "dddddd\"3p", "d\n|a")]
#[case("|a\nb\nc\nd\ne", "dddddd\"1p..", "d\nc\nb\n|a\ne")]
#[case("|a\nb\nc\nd\ne", "dddddd\"1pu.u.", "d\n|a\ne")]
#[case(
    "|a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk",
    "10dd\"1p",
    "k\n|a\nb\nc\nd\ne\nf\ng\nh\ni\nj"
)]
#[case(
    "|a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk",
    "dddddddddddddddddddd\"9p",
    "k\n|b"
)]
#[case("|ab\ncd\nef", "dj\"1p", "ef\n|ab\ncd")]
#[case("|ab cd\nef", "wd$\"1p\"-p", "ab c|d\nef")]
#[case("|ab\ncd\nef", "ldj\"1p", "ef\n|ab\ncd")]
#[case("|ab\ncd\nef", "yydd\"1p", "cd\n|ab\nef")]
#[case("|ab\ncd\nef", "\"addj\"1p", "cd\nef\n|ab")]
#[case("|ab\ncd\nef", "\"Addj\"1p", "cd\nef\n|ab")]
#[case("|ab\ncd\nef", "\"Addjp", "cd\nef\n|ab")]
#[case("|ab\ncd\nef", "\"addjp", "cd\nef\n|ab")]
// "-: el borrado chico, sin registro y dentro de una línea
#[case("|abc def", "dw\"-p", "dabc| ef")]
#[case("|abc def", "x\"axw\"-p", "c d|aef")]
#[case("|abc def", "yw\"adw\"-p", "|def")]
#[case("|abc def", "ciwX<Esc>\"-p", "Xab|c def")]
#[case("|ab\ncd", "dd\"-p", "|cd")]
#[case("|abc def", "dwx\"-p", "e|df")]
// "_: el agujero negro
#[case("|abc def", "yw\"_dwP", "abc| def")]
#[case("|abc def", "x\"_xp", "c|a def")]
#[case("|abc", "\"_yl\"_p", "|abc")]
#[case("|abc", "\"_x\"-p", "|bc")]
// ". y "/ son de solo lectura
#[case("|abc def", "\".dwx", "|bc def")]
#[case("|abc", "\".yy\"0p", "|abc")]
#[case("|abc def", "\"/p", "|abc def")]
#[case("|abc", "\".x", "|abc")]
// ". pega lo último insertado tecleándolo de nuevo
#[case("|abc", "ihola<Esc>\".p", "holahol|aabc")]
#[case("|abc", "ihola<Esc>$\".P", "holaabhol|ac")]
#[case("|abc", "ihola<Esc>3\".p", "holaholaholahol|aabc")]
#[case("|abc", "ihola<Esc>\".p.", "holaholahol|aabc")]
#[case("|abc", "iab<BS>c<Esc>\".p", "aca|cabc")]
#[case("|abc", "\".px", "|bc")]
#[case("|abc", "cwxy<Esc>\".P", "xx|yy")]
#[case("|abc", "ox<CR>y<Esc>gg\".p", "ax\n|ybc\nx\ny")]
#[case("|abc", "ihola<Esc>\".pu", "hol|aabc")]
#[case("|abc", "ihola<Esc>\".p.u", "holahola|abc")]
#[case("|abc", "ihola<Esc>\".pu<C-r>", "hol|aholaabc")]
// registros que no existen cancelan
#[case("|abc def", "\"!x", "|bc def")]
#[case("|abc def", "\"!p", "|abc def")]
#[case("|abc def", "\"<Esc>x", "|bc def")]
#[case("|abc def", "d\"ax", "ax|bc def")]
// . usa el registro del cambio
#[case("|ab cd ef", "\"adw.\"ap", "ecd| f")]
#[case("|ab cd ef", "\"Adw.\"ap", "eab cd| f")]
#[case("|ab\ncd", "\"ayyj\"ap.", "ab\ncd\nab\n|ab")]
#[case("|ab\ncd", "\"ayy\"bdd.", "|")]
// multibyte
#[case("|ñandú añil", "\"ayiww\"Ayiw$\"ap", "ñandú añilñandúañi|l")]
#[case("|aé\nb", "\"ax\"bdd\"ap\"bp", "ba\n|é")]
// "/ es el último patrón, de una búsqueda o de :s, y una búsqueda como motion usa el registro
#[case("|ab cd ab", "/cd<CR>\"/P", "ab c|dcd ab")]
#[case("|ab cd ab", "*$\"/p", "ab cd ab\\<ab\\|>")]
#[case("|ab cd", ":s/b/x/<CR>\"/p", "a|bx cd")]
#[case("|ab cd ab", "\"ad/ab<CR>$\"ap", "abab cd| ")]
#[case("|ab cd ab", "/ab<CR>gg\"bdn$\"bp", "abab cd| ")]
#[case("|ab cd ab", "\"cd*$\"cp", "abab cd| ")]
#[case("|ab\ncd\nab", "\"Ad/ab<CR>\"ap", "ab\n|ab\ncd")]
#[case("|ab cd", "\"_d/cd<CR>p", "|cd")]
// macros: q{a-z} graba, q termina, @{a-z} reproduce
#[case("|ab\ncd\nef", "qaxjq@a", "b\nd\n|ef")]
#[case("|ab\ncd\nef", "qaxjq@a@@", "b\nd\n|f")]
#[case("|ab\ncd\nef\ngh", "qaxjq2@a", "b\nd\nf\n|gh")]
#[case("|abc def ghi", "qadwq@a", "|ghi")]
#[case("|ab\ncd", "qaA!<Esc>jq@a", "ab!\ncd|!")]
#[case("|foo bar baz", "qaciwX<Esc>wq@a", "X X |baz")]
#[case("|abcdef", "qa2xq@a", "|ef")]
#[case("|ab\ncd", "qaxjq\"ap", "b\ncx|jd")]
#[case("|ab\ncd", "qaxq$p", "b|a\ncd")]
#[case("|ab\ncd", "qaxqqAjq@a", "b\n|d")]
#[case("|ab\ncd\nef", "qAxjq@a", "b\nd\n|ef")]
// una macro que falla se detiene ahí, aunque falte count
#[case("|ab\ncd\nef", "qaxjq5@a", "b\nd\n|f")]
#[case("|ab\ncd\nef", "qaxhjq@a", "b\n|d\nef")]
#[case("|abcd", "qax\\xq@a", "|d")]
#[case("|ab cd\nef", "qafcxq@a", "ab |d\nef")]
#[case("|ab\nc\nef", "qa;xq@a", "|b\nc\nef")]
#[case("|ab\ncd\nef", "qadjq@a", "|ef")]
#[case("|ab\ncd", "qa\"bpxq@a", "|b\ncd")]
// una macro recursiva se detiene al fallar
#[case("|ab\ncd\nef", "qaqqa0xj@aq@a", "b\nd\n|f")]
#[case("|ab\ncd\nef", "qaqqa0xj@aqu@a", "b\nd\n|f")]
#[case("|a1\nb2\nc3\nd4", "qaqqa$xj@aqgg@a", "\nb\nc\n|d")]
// una macro que usa .
#[case("|ab\ncd", "ix<Esc>qa.jq@a", "xxab\n|xcd")]
#[case("|ab\ncd\nef", "qaxj.q@a", "b\n\n|f")]
#[case("|abcdef", "xqa..q@a", "|f")]
// @@ sin macro previa no hace nada; @ con un registro vacío tampoco
#[case("|abc", "@@x", "|bc")]
#[case("|abc", "@bx", "|bc")]
#[case("|abc", "qbq@bx", "|bc")]
#[case("|abc", "q!x", "|bc")]
#[case("|abc", "@!x", "|bc")]
// la reproducción entera es un solo paso de undo
#[case("|abcdef", "qaxxq@au", "|cdef")]
#[case("|abcdef", "qaxxq2@au", "|cdef")]
#[case("|ab\ncd\nef", "qaxj.q@au", "b\n|d\nef")]
#[case("|ab\ncd\nef", "qaxjqj@au", "b\ncd\n|ef")]
#[case("|ab\ncd\nef", "qaxjq@auu", "|ab\ncd\nef")]
#[case("|abcdef", "qaxxq@au<C-r>", "|ef")]
#[case("|abcdef", "qaxuxq@a", "|cdef")]
// el texto de un registro también se reproduce
#[case("|jx\nab\ncd", "\"ay$j@a", "jx\nab\n|d")]
#[case("|ux\nabc", "\"ay$j@a", "ux\n|abc")]
#[case("|abc\ncd", "\"ayy@a", "abc\n|bc\ncd")]
#[case("|ihi\nab", "\"ay$j@a<Esc>u", "ihi\n|ab")]
#[case("|x\nab\ncd", "\"ayyj@a", "x\nb\n|cd")]
// multibyte
#[case("|aé\nbñ\ncú", "qa$xjq@a", "a\nb\n|cú")]
#[case("|ñandú\nañil", "qafdxjq@a", "ñanú\nañi|l")]
// una búsqueda o un :s que no encuentran nada cortan la macro
#[case("|a x b x c x", "qa/x<CR>xq9@a", "a  b  c| ")]
#[case("|a x b\nc x", "qa/x<CR>xq@aix<Esc>", "a  b\nc|x ")]
#[case("|ab\nab\ncd\nab", "qa:s/a/X/<CR>jq9@a", "Xb\nXb\n|cd\nab")]
#[case("|ab x\ncd x", "/x<CR>ggqanxq@a@@", "ab| \ncd ")]
#[case("|ab\ncd", "qa/zz<CR>xq@a", "|b\ncd")]
// ctrl-j envía la línea de comandos: un registro por líneas con :s o / se ejecuta
#[case("|:s/a/X/\nab a", "\"ayyj@a", ":s/a/X/\n|Xb a")]
#[case("|/c\nab c", "\"ayyj0@ax", "/c\nab| ")]
#[case("|ab cd", ":s/b/Z/<C-j>", "|aZ cd")]
// marks: m{a-z} pone, '{a-z} va al primer no blanco de su línea y `{a-z} a su posición
#[case("ab\n  c|d\nef", "maggG'a", "ab\n  |cd\nef")]
#[case("ab\n  c|d\nef", "maggG`a", "ab\n  c|d\nef")]
#[case("ab\nc|d\nef", "mbG`bjmaggk'a", "ab\ncd\n|ef")]
#[case("|ab", "'a", "|ab")]
#[case("|ab", "`ax", "|b")]
#[case("|ab", "m!x", "|b")]
#[case("|ab\ncd", "ma'Ax", "|b\ncd")]
#[case("ab\n|cd", "maggx'a", "b\n|cd")]
#[case("ab|c\nd", "majdd`a", "ab|c")]
// con operador son motions: ' por líneas y ` exclusiva
#[case("ab\nbb\ncc\nd|d", "kkmajjd'a", "a|b")]
#[case("ab\nb|b\ncc\ndd", "majjy`aP", "ab\nb|b\ncc\ndb\ncc\ndd")]
#[case("ab\nbb\ncc\nd|d", "kkmajjy'ap", "ab\nbb\n|bb\ncc\ndd\ncc\ndd")]
#[case("abc d|ef", "ma0d`a", "|ef")]
#[case("abc d|ef", "ma0c`aX<Esc>", "|Xef")]
#[case("|abc def", "ma$d`a", "|f")]
#[case("ab\nb|b\ncc\ndd", "majjd`a", "ab\nb|d")]
#[case("ab\nb|b\ncc\ndd", "majjd`ap", "ab\nbd|b\ncc\nd")]
#[case("ab\nbb\ncc\nd|d", "kkmajjd'aux", "ab\n|b\ncc\ndd")]
#[case("|ab", "d'ax", "|b")]
// `` d`a `` va a "1 aunque no pase de una línea, y también a "-
#[case("abc |def", "ma0d`a\"1p", "dabc| ef")]
#[case("abc |def", "ma0d`a\"-p", "dabc| ef")]
#[case("abc |def", "ma0d`a$p", "defabc| ")]
// también lo borrado con una búsqueda, como el `use_reg_one` de vim
#[case("|ab cd ab", "d/cd<CR>\"1p", "cab| d ab")]
#[case("|ab cd ab", "d*\"1p", "aab cd| b")]
#[case("|ab cd ab", "dw\"1p", "|cd ab")]
// . repite con la mark donde esté
#[case("|ab\ncd\nef\ngh\nij", "jjmaggd'aj.", "gh\n|ij")]
// '' y `` vuelven a donde se estaba antes del salto
#[case("|a\nb\nc", "G''", "|a\nb\nc")]
#[case("|a\nb\nc", "jlG``", "a\n|b\nc")]
#[case("a|b\nb\nc", "Gx``", "a|b\nb\n")]
#[case("a|b\nb\nc", "''", "a|b\nb\nc")]
#[case("a|b\nb\nc", "jj''", "ab\nb\n|c")]
#[case("a|b\nb\nc", "mbG'b''", "ab\nb\n|c")]
#[case("a|b\nb\nc\nd", "G''''", "ab\nb\nc\n|d")]
#[case("a\n|b\nc\nd", "yGjj''", "a\n|b\nc\nd")]
#[case("a\n|b\nc\nd", "ggjj''", "a\n|b\nc\nd")]
#[case("a\n|b\nc\nd", "majj'a``", "a\nb\nc\n|d")]
#[case("a\n|b\nc\nd", "m'G''", "a\n|b\nc\nd")]
#[case("a\n|b\nc\nd", "Gd''", "|a")]
#[case("|a\nb\nc\nd", "Gy``P", "|a\nb\nc\na\nb\nc\nd")]
// siguen al texto cuando se edita arriba de ellas
#[case("a\nbb\nc|c", "maggddG0`a", "bb\nc|c")]
#[case("a\nbb\nc|c", "maggOx<Esc>G0`a", "x\na\nbb\nc|c")]
#[case("a\nbb\nc|c", "maggyyPG0'a", "a\na\nbb\n|cc")]
#[case("a\nb|b\ncc\ndd", "majddgg`a", "a\nb|b\ndd")]
#[case("a\nbb\nc|c\ndd", "maggdjG`a", "c|c\ndd")]
#[case("a\nbb\nc|c", "magg2ix<CR><Esc>G`a", "x\nx\na\nbb\nc|c")]
#[case("a\nbb\nc|c", "maggo<Esc>o<Esc>G`a", "a\n\n\nbb\nc|c")]
#[case("a\nbb\nc|c", "maggddu`a", "a\nbb\nc|c")]
#[case("a\nbb\nc|c", "maggddu<C-r>gg`a", "bb\nc|c")]
// en su misma línea conservan la columna
#[case("abc |def", "ma0iXX<Esc>$`a", "XXab|c def")]
#[case("abc |def", "ma0x$`a", "bc d|ef")]
#[case("abc |def", "mad0$`a", "de|f")]
#[case("abcde|f", "ma0dw`a", "|")]
// si se borra su línea se pierden, y el undo las devuelve
#[case("a\nb|b\ncc\ndd", "maddgg`a", "|a\ncc\ndd")]
#[case("a\nb|b\ncc\ndd", "madd'a", "a\nc|c\ndd")]
#[case("a\nb|b\ncc\ndd", "maddugg`a", "a\nb|b\ncc\ndd")]
#[case("a\nbb\ncc\nd|d", "maddgg`a", "|a\nbb\ncc")]
#[case("a\nb|b\ncc\ndd", "makdjgg`a", "c|c\ndd")]
#[case("a\nb|b", "maggdG`a", "|")]
#[case("|ab", "maddix<Esc>`a", "|x")]
// borrar una línea no se lleva la mark de la línea vacía de abajo
#[case("x\ny\n|\nz", "makddgg'a", "x\n|\nz")]
#[case("x\n\n|\ny", "makddgg'a", "x\n|\ny")]
#[case("x\ny\n|\nz", "makddG`a", "x\n|\nz")]
// si su línea se une a la anterior, se va con su texto
#[case("ab\nc|d", "ma0i<BS><Esc>0`a", "abc|d")]
#[case("ab\nc|d", "makA<CR><Esc>G0`a", "ab\n\nc|d")]
// multibyte
#[case("añ\nbé|ú", "maggddx`a", "b|é")]
#[case("añ\nbé|ú", "maggiñ<CR><Esc>G0`a", "añ\nñ\nbé|ú")]
#[case("aé|ú", "ma0x$`a", "é|ú")]
#[case("ñ|aé", "ma0dl`a", "a|é")]
// línea de comandos: entrar, editar y salir sin ejecutar nada
#[case("ab|c", ":w", "ab|c")]
#[case("ab|c", ":wq<BS>", "ab|c")]
#[case("ab|c", ":<Esc>", "ab|c")]
#[case("ab|c", ":w<BS><BS>", "ab|c")]
#[case("ab|c", "3:<Esc>", "ab|c")]
#[case("ab|c", ":q<Esc>x", "a|b")]
#[case("ab|c", ":<BS>x", "a|b")]
#[case("|abc", "i:<Esc>", "|:abc")]
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
#[case("|ab\n|cd", "2ix<Esc>", "x|xab\nx|xcd")]
#[case("|ab\n|cd", "x.", "|\n|")]
#[case("|ab\n|cd", "A!<Esc>.", "ab!|!\ncd!|!")]
#[case("f|oo b|ar", "ciwX<Esc>.", "|X |X")]
#[case("foo |qux foo |qux", "d2aw", "foo |qu|x")]
#[case("x|yzw|a", "dfa", "|x")]
#[case("|\n|ab", "xp", "|\na\n|b\na")]
fn multiple_cursors(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).text, after);
}

#[test]
fn reports_what_each_key_did() {
    use nota_vim::{Action, InsertAt, Key, KeyOutcome, Motion, Vim};

    let (mut buffer, mut selections) = support::parse("|ab");
    let mut vim = Vim::new();
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
    let mut vim = Vim::new();
    let delete_right = Action::from_name("vim::delete_right").unwrap();
    vim.execute(delete_right, &mut buffer, &mut selections)
        .unwrap();
    assert_eq!(support::render(&buffer, [selections.primary().head]), "|b");
}

#[test]
fn operators_wait_in_their_own_mode() {
    use nota_vim::{Key, Mode, Operator, Vim};

    let (mut buffer, mut selections) = support::parse("|ab");
    let mut vim = Vim::new();
    vim.feed(Key::Char('d'), &mut buffer, &mut selections)
        .unwrap();
    assert_eq!(vim.mode(), Mode::OperatorPending(Operator::Delete));
    vim.feed(Key::Escape, &mut buffer, &mut selections).unwrap();
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(buffer.to_string(), "ab");
}

#[test]
fn find_by_name_waits_for_the_char() {
    use nota_vim::{Action, CharSearch, Key, KeyOutcome, Vim};

    let (mut buffer, mut selections) = support::parse("|abc");
    let mut vim = Vim::new();
    let find = Action::from_name("vim::find_forward").unwrap();
    vim.execute(find, &mut buffer, &mut selections).unwrap();
    let outcome = vim
        .feed(Key::Char('c'), &mut buffer, &mut selections)
        .unwrap();
    let search = CharSearch {
        backward: false,
        till: false,
    };
    assert_eq!(outcome, KeyOutcome::Executed(Action::FindChar(search)));
    assert_eq!(
        support::render(&buffer, [selections.primary().head]),
        "ab|c"
    );
}

#[test]
fn esc_cancels_a_pending_find() {
    use nota_vim::{Action, Key, KeyOutcome, Vim};

    let (mut buffer, mut selections) = support::parse("|abc");
    let mut vim = Vim::new();
    vim.feed(Key::Char('f'), &mut buffer, &mut selections)
        .unwrap();
    let outcome = vim.feed(Key::Escape, &mut buffer, &mut selections).unwrap();
    assert_eq!(outcome, KeyOutcome::Executed(Action::NormalMode));
}

/// nvim no modela un movimiento del cursor por fuera del motor.
#[rstest]
#[case("f")]
#[case("3")]
#[case("d")]
#[case("g")]
fn moving_the_cursor_cancels_what_was_pending(#[case] pending: &str) {
    use nota_buffer::{ByteOffset, Selection, Selections};
    use nota_vim::{Mode, Vim};

    let (mut buffer, mut selections) = support::parse("|abc def");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, pending);
    selections = Selections::single(Selection::cursor(ByteOffset::new(4)));
    vim.cursor_moved();
    support::feed(&mut vim, &mut buffer, &mut selections, "x");
    assert_eq!(vim.mode(), Mode::Normal);
    let heads = selections.iter().map(|s| s.head);
    assert_eq!(support::render(&buffer, heads), "abc |ef");
}

#[test]
fn repeat_change_runs_by_name() {
    use nota_vim::{Action, Vim};

    let (mut buffer, mut selections) = support::parse("|abc");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "x");
    let repeat = Action::from_name("vim::repeat_change").unwrap();
    vim.execute(repeat, &mut buffer, &mut selections).unwrap();
    assert_eq!(support::render(&buffer, [selections.primary().head]), "|c");
}

/// Como un clic en Normal en vim: cancela lo pendiente, pero `.` sigue
/// repitiendo el último cambio.
#[test]
fn moving_the_cursor_in_normal_keeps_the_last_change() {
    use nota_buffer::{ByteOffset, Selection, Selections};
    use nota_vim::Vim;

    let (mut buffer, mut selections) = support::parse("|abc def");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "x");
    selections = Selections::single(Selection::cursor(ByteOffset::new(3)));
    vim.cursor_moved();
    support::feed(&mut vim, &mut buffer, &mut selections, ".");
    let heads = selections.iter().map(|s| s.head);
    assert_eq!(support::render(&buffer, heads), "bc |ef");
}
