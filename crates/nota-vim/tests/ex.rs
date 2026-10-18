#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Los comandos de `:` que corre el motor: rangos, `:d`, `:y`, `:m`, `:t`,
//! `:j`, `:>`, `:<`, `:sort`, `:normal`, `:g` y `@:`. Cada caso corre en el
//! motor y en `nvim --clean`; si nvim no coincide con `después`, el caso
//! está mal.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};

#[template]
#[rstest]
// rangos: números, `.`, `$`, desplazamientos, `;` y las dos últimas
#[case("a\nbb\ncc\ndd\ne|e", ":2<CR>", "a\nb|b\ncc\ndd\nee")]
#[case("a\nbb\n  cc\ndd\ne|e", ":3<CR>", "a\nbb\n | cc\ndd\nee")]
#[case("a\nbb\ncc\ndd\ne|e", ":99<CR>", "a\nbb\ncc\ndd\ne|e")]
#[case("a\nbb\ncc\ndd\ne|e", ":0<CR>", "|a\nbb\ncc\ndd\nee")]
#[case("a\nbb\ncc\ndd\ne|e", ":-2<CR>", "a\nbb\nc|c\ndd\nee")]
#[case("a\nbb\ncc\ndd\ne|e", ":1+<CR>", "a\nb|b\ncc\ndd\nee")]
#[case("a\nbb\ncc\ndd\ne|e", ":$-1<CR>", "a\nbb\ncc\nd|d\nee")]
#[case("a\nb|\nc\nd\ne", ":2;+1d<CR>", "a\n|d\ne")]
#[case("a\nbb\ncc\ndd\ne|e", ":2;+1y<CR>", "a\nb|b\ncc\ndd\nee")]
#[case("a\nb|\nc\nd\ne", ":1,2,4d<CR>", "a\n|e")]
#[case("a\nb|\nc\nd\ne", ":.,$d<CR>", "|a")]
#[case("a\nb|\nc\nd\ne", ":,+1d<CR>", "a\n|d\ne")]
#[case("a\nb|\nc\nd\ne", ":%d<CR>", "|")]
#[case("a\n|b\nc\nd\ne", ":9d<CR>", "a\n|b\nc\nd\ne")]
// rangos con marks y con la selección Visual
#[case("a\nb|\nc\nd\ne", "jmakk:'a,$d<CR>", "a\n|b")]
#[case("a\n|b\nc\nd\ne", ":'a<CR>", "a\n|b\nc\nd\ne")]
#[case("a\nb|\nc\nd\ne", "jVj<Esc>gg:'<lt>,'>d<CR>", "a\nb\n|e")]
#[case("a\nb|\nc\nd\ne", "Vj:d<CR>", "a\n|d\ne")]
#[case("a\nb|\nc\nd\ne", "vj:d<CR>", "a\n|d\ne")]
#[case("a\nb|\nc\nd\ne", "Vj:<BS><BS><BS><BS><BS>2d<CR>", "a\n|c\nd\ne")]
// rangos con patrones: siguen desde el cursor, dan la vuelta y quedan como búsqueda
#[case("a\nb|\nc\nd\ne", ":/d/d<CR>", "a\nb\nc\n|e")]
#[case("a\nb|\nc\nd\ne", ":/d<CR>", "a\nb\nc\n|d\ne")]
#[case("a\nb|\nc\nd\ne", ":?a?,/d/-1d<CR>", "|d\ne")]
#[case("a\nb|\nc\nd\ne", ":/a/d<CR>", "|b\nc\nd\ne")]
#[case("a\nb|\nc\nd\ne", ":/b/d<CR>", "a\n|c\nd\ne")]
#[case("a\nb|\nc\nd\ne", ":/c//d/d<CR>", "a\nb\nc\n|e")]
#[case("a\n|b\nc\nd\ne", ":/x/d<CR>", "a\n|b\nc\nd\ne")]
#[case("a\nbb\ncc\ndd\ne|e", ":/c<CR>n", "a\nbb\n|cc\ndd\nee")]
#[case("a\nbb\ncc\ndd\ne|e", ":?b?<CR>n", "a\n|bb\ncc\ndd\nee")]
#[case("ab\ncd\nab|\nx", "/ab<CR>://d<CR>", "ab\ncd\n|x")]
// :d y :y con registro y count
#[case("a|b\ncd\nef", ":2y a<CR>\"ap", "ab\n|cd\ncd\nef")]
#[case("a|b\ncd\nef", ":2,3d b<CR>\"bp", "ab\n|cd\nef")]
#[case("a|b\ncd\nef", ":d 2<CR>", "|ef")]
#[case("a|b\ncd\nef", ":d2<CR>", "|ef")]
#[case("a|b\ncd\nef", ":2d<CR>\"1p", "ab\nef\n|cd")]
#[case("a|b\ncd\nef", ":y A<CR>:2y A<CR>\"ap", "ab\n|ab\ncd\ncd\nef")]
#[case("a|b\ncd\nef", ":3d _<CR>p", "ab\n|cd")]
#[case("a|b\ncd\nef", ":2,3y<CR>Gp", "ab\ncd\nef\n|cd\nef")]
#[case("a|b\ncd\nef", ":y 9<CR>Gp", "ab\ncd\nef\n|ab\ncd\nef")]
#[case("a\nb|cdef\n   cc\nd", ":2d<CR>", "a\n   |cc\nd")]
#[case("a\nbb\ncc\ndd\ne|e", ":2d<CR>''", "a\ncc\ndd\n|ee")]
#[case("a|b\ncd\nef", ":2d<CR>u", "ab\nc|d\nef")]
// :m y :t
#[case("a\nb\nc|cc\nd", ":m0<CR>", "c|cc\na\nb\nd")]
#[case("a\nb\nc|cc\nd", ":t0<CR>", "c|cc\na\nb\nccc\nd")]
#[case("a\nb\nc|cc\nd", ":1,2m$<CR>", "ccc\nd\na\n|b")]
#[case("a\nbb\ncc\ndd\ne|e", ":1m3<CR>", "bb\ncc\n|a\ndd\nee")]
#[case("a\nb\nc|cc\nd", ":m+1<CR>", "a\nb\nd\nc|cc")]
#[case("a\nb\nc|cc\nd", ":m-2<CR>", "a\nc|cc\nb\nd")]
#[case("a\nb\nc|cc\nd", ":m2<CR>", "a\nb\nc|cc\nd")]
#[case("a\nb\nc|cc\nd", ":m3<CR>", "a\nb\nc|cc\nd")]
#[case("a\nb\nc|cc\nd", ":1,3m1<CR>", "a\nb\nc|cc\nd")]
#[case("a\nb\nc|cc\nd", ":$m0<CR>", "|d\na\nb\nccc")]
#[case("a\nb\nc|cc\nd", ":1,2t$<CR>", "a\nb\nccc\nd\na\n|b")]
#[case("a\nb\nc|cc\nd", ":1,3t2<CR>", "a\nb\na\nb\nc|cc\nccc\nd")]
#[case("a\nb\nc|cc\nd", ":co.<CR>", "a\nb\nccc\nc|cc\nd")]
#[case("a\nb\nc|cc\nd", ":m0<CR>u", "a\nb\nc|cc\nd")]
// :j
#[case("a|b\n   cd\n\nx", ":j<CR>", "|ab cd\n\nx")]
#[case("a|b\n   cd\n\nx", ":j!<CR>", "|ab   cd\n\nx")]
#[case("a|b\n   cd\n\nx", ":%j<CR>", "|ab cd x")]
#[case("a|b\n   cd\n\nx", ":j 3<CR>", "|ab cd\nx")]
#[case("a|b \ncd\n)x\n\tq", ":j4<CR>", "|ab cd)x q")]
#[case("ab\ncd\n|x", ":j<CR>", "ab\ncd\n|x")]
#[case("ab\nc|d\nx", ":2,2j<CR>", "ab\nc|d\nx")]
#[case("ab.\n|cd", ":1j<CR>", "|ab. cd")]
#[case("  a|b\ncd", ":j<CR>", "  |ab cd")]
#[case("a|b\ncd", ":j<CR>u", "a|b\ncd")]
// :> y :<
#[case("  x\nab|cdef", ":><CR>", "  x\n\t|abcdef")]
#[case("  x\nabcd|ef", ":1><CR>", "\t  |x\nabcdef")]
#[case("  x\nabcd|ef", ":1,2><CR>", "\t  x\n\t|abcdef")]
#[case("    abcdef|g\nx", ":><CR>", "\t    |abcdefg\nx")]
#[case("    abc\n\n  |x", ":%><CR>", "\t    abc\n\n\t  |x")]
#[case("ab|c\n   \nx", ":%>>><CR>", "\t\t\tabc\n\t\t\t   \n\t\t\t|x")]
#[case("a|bc\nx", ":> 2<CR>", "\tabc\n\t|x")]
#[case("\tabcdef|g\nx", ":<lt><CR>", "|abcdefg\nx")]
#[case("\t\tab|c", ":<lt><lt><CR>", "|abc")]
#[case("\t  ab|c", ":<lt><CR>", "  |abc")]
#[case("           ab|c", ":<lt><CR>", "   |abc")]
#[case("a\nbb\ncc\ndd\ne|e", ":1,3><CR>''", "\ta\n\tbb\n\tcc\ndd\n|ee")]
// :sort
#[case("c\nB\na|\nb\nA", ":sort<CR>", "|A\nB\na\nb\nc")]
#[case("c\nB\na|\nb\nA", ":sor i<CR>", "|a\nA\nB\nb\nc")]
#[case("c\nB\na|\nb\nA", ":sort!<CR>", "|c\nb\na\nB\nA")]
#[case("c\nb\na|\nb\nB", ":sort u<CR>", "|B\na\nb\nc")]
#[case("c\nb\na|\nb\nB", ":sort ui<CR>", "|a\nb\nc")]
#[case("x10\nx9\n|y\nx-3\nz\n2", ":sort n<CR>", "|y\nz\nx-3\n2\nx9\nx10")]
#[case("x10\nx9\n|y\nx-3\nz\n2", ":sort! n<CR>", "|x10\nx9\n2\nx-3\nz\ny")]
#[case("a3 z\nb1 y\n|c2 x", ":sort /[a-z]/<CR>", "|b1 y\nc2 x\na3 z")]
#[case("a3 z\nb1 y\n|c2 x", ":sort /\\d/ r<CR>", "|b1 y\nc2 x\na3 z")]
#[case("a3 z\nb1 y\n|c2 x\nq", ":sort /\\d /<CR>", "|q\nc2 x\nb1 y\na3 z")]
#[case("a3 z\nb1 y\n|c2 x\nq", ":sort! /\\d /<CR>", "|a3 z\nb1 y\nc2 x\nq")]
#[case("  b\n|a\nc", ":sort<CR>", "  |b\na\nc")]
#[case("b\n|a\nb\nc\nb", ":2,4sort<CR>", "b\n|a\nb\nc\nb")]
#[case("b\n|a\nc", ":sort<CR>u", "b\n|a\nc")]
// :normal, con y sin rango
#[case("a|bc\nd", ":normal ihi<CR>", "ah|ibc\nd")]
#[case("a|bc\nd", ":normal 3ihi<CR>", "ahihih|ibc\nd")]
#[case("a|bc\nd", ":%norm Ax<CR>", "abcx\nd|x")]
#[case("a|bc\nd", ":norm d<CR>", "a|bc\nd")]
#[case("a|bc\nd", ":norm dw<CR>", "|a\nd")]
#[case("ab|c\nd", ":norm :s/a/x/<CR>", "ab|c\nd")]
#[case("ab|c\nd", ":%norm hx<CR>", "abc\n|d")]
#[case("ab|c\nd", ":norm! lx<CR>", "ab|c\nd")]
#[case("abc\nd|\ne", ":%norm x<CR>", "bc\n\n|")]
#[case("abc\nd|\ne", ":%norm Ax<CR>u", "ab|c\nd\ne")]
#[case("ab|c\nab\nab", ":2,3norm f;x<CR>", "abc\nab\n|ab")]
#[case("ab|c\nab\nab", ":2,$norm 0\"ayl<CR>\"ap", "abc\nab\na|ab")]
#[case("one\ntwo|\nthree", ":%norm dd<CR>", "|")]
#[case("x|\na\nb", "qq0iz<Esc>jq:2,3norm @q<CR>", "zx\nza\n|zb")]
// :g, :g! y :v
#[case("a1\nb\na|2\nc\na3", ":g/a/d<CR>", "b\n|c")]
#[case("a1\nb\na|2\nc\na3", ":v/a/d<CR>", "a1\na2\n|a3")]
#[case("a1\nb\na|2\nc\na3", ":g!/a/d<CR>", "a1\na2\n|a3")]
#[case("a1\nb\na|2\nc\na3", ":g/a/m0<CR>", "a|3\na2\na1\nb\nc")]
#[case("a1\nb\na|2\nc\na3", ":g/a/s/a/x/<CR>", "x1\nb\nx2\nc\n|x3")]
#[case("a1\nb\na|2\nc\na3", ":g/a/normal Ax<CR>", "a1x\nb\na2x\nc\na3|x")]
#[case("a1\nb\na|2\nc\na3", ":g/a/<CR>", "a1\nb\na2\nc\na|3")]
#[case("a1\nb\na|2\nc\na3", ":g/zz/d<CR>", "a1\nb\na|2\nc\na3")]
#[case("a1\nb\na|2\nc\na3", ":g/a/t.<CR>", "a1\na1\nb\na2\na2\nc\na3\na|3")]
#[case("a1\nb\na|2\nc\na3", ":g/a/s/1/X/<CR>", "aX\nb\na2\nc\n|a3")]
#[case("a1\nb\nab|2\nc\n  a3", ":g/a/s/1/X/<CR>", "aX\nb\nab2\nc\n  |a3")]
#[case("a1\nb\nab|2\nc\na3", ":g/a/normal $x<CR>", "a\nb\nab\nc\n|a")]
#[case("a1\nb\na|2\nc\na3", ":2,4g/a/d<CR>", "a1\nb\n|c\na3")]
#[case("a1\nb\na|2\nc\na3", "/b<CR>:g//d<CR>", "a1\n|a2\nc\na3")]
#[case("a1\nb\na|2\nc\na3", ":g/a<CR>n", "|a1\nb\na2\nc\na3")]
#[case("a1\nb\na|2\nc\na3", ":g/^/m0<CR>", "a|3\nc\na2\nb\na1")]
#[case("a1\nb\na|2\nc\na3", ":g/a/j<CR>", "a1 b\na2 c\n|a3")]
#[case("a1\nb\na|2\nc\na3", ":g/a/g/1/d<CR>", "b\na2\nc\n|a3")]
#[case("a1\nb\na|2\nc\na3", ":g/./d<CR>u", "a|1\nb\na2\nc\na3")]
#[case("a1\nb\na|2\nc\na3", ":g/a/s/a/x/<CR>u", "|a1\nb\na2\nc\na3")]
#[case("a1\nb\na|2\nc\na3", ":g/a/normal ddp<CR>", "b\na1\nc\na2\n|a3")]
// @: repite la última línea de `:`, y `@@` también
#[case("a|a\nbb\ncc\ndd\nee", ":s/./X/<CR>j@:", "Xa\n|Xb\ncc\ndd\nee")]
#[case("a|a\nbb\ncc\ndd\nee", ":s/./X/<CR>j@:j@@", "Xa\nXb\n|Xc\ndd\nee")]
#[case("a|a\nbb\ncc\ndd\nee", ":d<CR>2@:", "|dd\nee")]
#[case("a|a\nbb\ncc\ndd\nee", ":d<CR>2@:u", "|bb\ncc\ndd\nee")]
#[case("a|a\nbb\ncc\ndd\nee", ":s/./X/<CR>qqj@:q@q", "Xa\nXb\n|Xc\ndd\nee")]
#[case("a|a\nbb\ncc\ndd\nee", "@:x", "|a\nbb\ncc\ndd\nee")]
#[case("a|a\nbb\ncc\ndd\nee", ":zz<CR>:d<CR>@:", "|cc\ndd\nee")]
#[case("a|a\nbb\ncc\ndd\nee", ":%s/./X/<CR>u@:", "Xa\nXb\nXc\nXd\n|Xe")]
// un solo paso de undo por comando, también dentro de una macro, y a dónde
// vuelve el cursor
#[case("a\nbb\nc|cc\nd", ":t0<CR>u", "|a\nbb\nccc\nd")]
#[case("a\nbb\nc|cc\nd", ":1t$<CR>u", "a\nbb\nc|cc\nd")]
#[case("a\nbb\nc|cc\nd", ":1m$<CR>u", "|a\nbb\nccc\nd")]
#[case("a\nbb\nc|cc\nd", ":1,2><CR>u", "|a\nbb\nccc\nd")]
#[case("a\nbb\nc|cc\nd", ":4><CR>u", "a\nbb\nccc\n|d")]
#[case("xa\nbb\nc|cc\nd", ":1d<CR>u", "x|a\nbb\nccc\nd")]
#[case("b\nd\na\nc|cc", ":sort<CR>u", "b\nd\na\nc|cc")]
#[case("b\nd\na\nc|cc", ":1,2sort!<CR>u", "|b\nd\na\nccc")]
#[case("ab\nb|b\ncc", ":1,2j<CR>u", "a|b\nbb\ncc")]
#[case("abc\nb|b\ncc", ":%norm Ax<CR>u", "ab|c\nbb\ncc")]
#[case("a|a\nbb\ncc", "qq:%norm Ax<CR>:2d<CR>q:1d<CR>u", "|aax\nccx")]
#[case("a|a\nbb\ncc", "qq:%norm Ax<CR>:2d<CR>qu@qu", "aa|x\nbbx\nccx")]
fn cases(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {}

#[apply(cases)]
fn engine(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let engine = support::run_engine(before, keys);
    assert_eq!(engine.text, after);
    assert_eq!(engine.mode, "n");
}

#[apply(cases)]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_oracle(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let nvim = support::nvim::run(before, keys);
    assert_eq!(nvim.text, after, "la tabla no coincide con nvim");
    assert_eq!(nvim.mode, "n");
}
