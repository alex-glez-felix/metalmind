#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! La jumplist (`ctrl-o`, `ctrl-i`), `%` y las marks `A`–`Z` dentro de un
//! mismo archivo. Cada caso corre en el motor y en `nvim --clean`.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_vim::{CommandError, KeyOutcome, Vim};
use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};

const LINES: &str = "|l1\nl2\nl3\nl4\nl5\nl6";

#[template]
#[rstest]
// ctrl-o vuelve y ctrl-i (o tab) avanza
#[case(LINES, "G<C-o>", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "G<C-o><C-i>", "l1\nl2\nl3\nl4\nl5\n|l6")]
#[case(LINES, "G<C-o><Tab>", "l1\nl2\nl3\nl4\nl5\n|l6")]
#[case(LINES, "<C-o>", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "G<C-i>", "l1\nl2\nl3\nl4\nl5\n|l6")]
#[case(LINES, "3G5Ggg<C-o><C-o>", "l1\nl2\n|l3\nl4\nl5\nl6")]
#[case(LINES, "3G5Ggg2<C-o>", "l1\nl2\n|l3\nl4\nl5\nl6")]
#[case(LINES, "3G5Ggg3<C-o><C-i>", "l1\nl2\nl3\nl4\n|l5\nl6")]
#[case(LINES, "3G5Ggg9<C-o>", "|l1\nl2\nl3\nl4\nl5\nl6")]
// de la misma línea queda la última entrada
#[case(LINES, "Ggg<C-o><C-o>", "l1\nl2\nl3\nl4\nl5\n|l6")]
#[case(LINES, "GggGgg<C-o><C-o><C-o>", "l1\nl2\nl3\nl4\nl5\n|l6")]
#[case(LINES, "3GggG<C-o><C-o><C-o><C-i><C-i>", "l1\nl2\nl3\nl4\nl5\n|l6")]
// la columna se conserva
#[case("ab|cd\nef\ngh", "G<C-o>", "ab|cd\nef\ngh")]
#[case("ab|cd\nef\ngh", "G<C-o>j", "abcd\ne|f\ngh")]
#[case("abcd\nef\ngh|ij", "gg<C-o>", "abcd\nef\ngh|ij")]
// las entradas siguen a su línea cuando se edita
#[case(LINES, "jjjGggdd<C-o>", "l2\nl3\nl4\nl5\n|l6")]
#[case(LINES, "jjjGggdd<C-o><C-o>", "l2\nl3\n|l4\nl5\nl6")]
#[case(LINES, "jjjGggOx<Esc><C-o><C-o>", "x\nl1\nl2\nl3\n|l4\nl5\nl6")]
#[case(LINES, "jjjGkdd<C-o>", "l1\nl2\nl3\n|l4\nl6")]
#[case(LINES, "jjjGggjdj<C-o>", "l1\nl4\nl5\n|l6")]
#[case(LINES, "Gggu<C-o>", "l1\nl2\nl3\nl4\nl5\n|l6")]
// las marks, las búsquedas, `%`, H, M y L son saltos; `''` vuelve
#[case(LINES, "jjjmaggj'a<C-o>", "l1\n|l2\nl3\nl4\nl5\nl6")]
#[case(LINES, "jjjmaggj`a<C-o>", "l1\n|l2\nl3\nl4\nl5\nl6")]
#[case(LINES, "/l4<CR><C-o>", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "/l4<CR>?l2<CR><C-o>", "l1\nl2\nl3\n|l4\nl5\nl6")]
#[case(LINES, "/l<CR>nn<C-o><C-o>", "l1\n|l2\nl3\nl4\nl5\nl6")]
#[case(LINES, "/l<CR>nN<C-o>", "l1\nl2\n|l3\nl4\nl5\nl6")]
#[case("|ab\ncd\nab", "*<C-o>", "|ab\ncd\nab")]
#[case("ab\ncd\n|ab", "#<C-o>", "ab\ncd\n|ab")]
#[case("|ab\ncd\nab", "/zz<CR><C-o>", "|ab\ncd\nab")]
#[case("|(a\nb)\nc", "%<C-o>", "|(a\nb)\nc")]
#[case("|(a\nb)\nc", "%<C-o><C-i>", "(a\nb|)\nc")]
#[case("|ab\ncd", "%<C-o>", "|ab\ncd")]
#[case(LINES, "L<C-o>", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "G''", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "Ggg<C-o>''", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "jjjGggu<C-o>''", "|l1\nl2\nl3\nl4\nl5\nl6")]
// `:N` no es un salto en nvim; `:g` es uno solo
#[case(LINES, "G:3<CR>''", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "G:3<CR><C-o>", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "G:g/l/s/l/x/<CR><C-o>", "|x1\nx2\nx3\nx4\nx5\nx6")]
#[case(LINES, "G:g/l[24]/d<CR><C-o><C-o>", "|l1\nl3\nl5\nl6")]
// un operador con ctrl-o falla
#[case(LINES, "Ggg<C-o>dd", "l1\nl2\nl3\nl4\n|l5")]
#[case(LINES, "Gd<C-o>", "l1\nl2\nl3\nl4\nl5\n|l6")]
// % empareja (), [] y {}
#[case("|(a)", "%", "(a|)")]
#[case("(a|)", "%", "|(a)")]
#[case("x |[a [b] c]", "%", "x [a [b] c|]")]
#[case("|x {a\nb}", "%", "x {a\nb|}")]
#[case("a|b", "%", "a|b")]
#[case("|(a", "%", "|(a")]
#[case("|( \")\" )", "%", "( \")\" |)")]
#[case("|( \")\" \" )", "%", "( \"|)\" \" )")]
#[case("|( ')' )", "%", "( ')' |)")]
#[case("|(\\) )", "%", "(\\) |)")]
#[case("|\\( ) \\)", "%", "\\( ) \\|)")]
#[case("|(ab)c", "d%", "|c")]
#[case("|(ab\ncd)e", "d%", "|e")]
#[case("|ab(c)d", "y%P", "ab(c|)ab(c)d")]
#[case("(a|b)", "v%d", "|)")]
#[case(LINES, "50%", "l1\nl2\n|l3\nl4\nl5\nl6")]
#[case(LINES, "100%", "l1\nl2\nl3\nl4\nl5\n|l6")]
#[case(LINES, "101%", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "50%<C-o>", "|l1\nl2\nl3\nl4\nl5\nl6")]
#[case(LINES, "d50%", "|l4\nl5\nl6")]
// las marks A–Z del mismo archivo son como las a–z
#[case(LINES, "jjmAG'A", "l1\nl2\n|l3\nl4\nl5\nl6")]
#[case("ab\nc|d\nef", "mBgg`B", "ab\nc|d\nef")]
#[case(LINES, "jjmAGd'A", "l1\n|l2")]
#[case(LINES, "jjmAggdd'A", "l2\n|l3\nl4\nl5\nl6")]
#[case(LINES, "jjmAjdkG'A", "l1\nl2\n|l5\nl6")]
#[case(LINES, "jjmA:'A,'Ad<CR>", "l1\nl2\n|l4\nl5\nl6")]
#[case(LINES, "jjmAG'A<C-o>", "l1\nl2\nl3\nl4\nl5\n|l6")]
// y siguen a lo editado antes en la misma macro o el mismo :g
#[case(LINES, "jjjmAggqqdd'Aq", "l2\nl3\n|l4\nl5\nl6")]
#[case(LINES, "jjjmAggqqdd'Aqgg@q", "l3\n|l4\nl5\nl6")]
#[case(LINES, "jjjmAgg:g/l[12]/'Ad<CR>", "l1\nl2\nl3\n|l6")]
#[case(LINES, "jjmAgg:normal dd'Ax<CR>", "l2\n|3\nl4\nl5\nl6")]
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

/// Una mark que no está puesta es `E20`, también una `A`–`Z`.
#[rstest]
#[case("'z")]
#[case("`z")]
#[case("'A")]
#[case("d'Q")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn una_mark_sin_poner_es_e20(#[case] keys: &str) {
    let (mut buffer, mut selections) = support::parse(LINES);
    let mut vim = Vim::new();
    let last = support::feed(&mut vim, &mut buffer, &mut selections, keys);
    let error = CommandError::MarkNotSet;
    assert_eq!(last.unwrap(), KeyOutcome::InvalidCommand(error.clone()));
    assert_eq!(support::nvim::message(LINES, keys), error.to_string());
}
