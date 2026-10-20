#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Visual por bloque (`ctrl-v`). Como en `visual.rs`, cada caso corre en el
//! motor y en `nvim --clean`; los que terminan en Visual comparan también el
//! otro extremo de la selección.

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
#[case("a|bc\ndef\nghi", "<C-v>jld", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlx", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl<Esc>x", "abc\nd|e\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl<C-c>x", "abc\nd|e\nghi")]
#[case("|abc\ndef", "<C-v><C-v>x", "|bc\ndef")]
#[case("|abc\ndef", "<C-v>jvd", "|ef")]
#[case("|abc\ndef", "<C-v>jVd", "|")]
#[case("a|bc\ndef", "vj<C-v>d", "a|c\ndf")]
#[case("a|bc\ndef", "Vj<C-v>d", "a|c\ndf")]
#[case("ab|c\ndef\nghi", "<C-v>jjhd", "|a\nd\ng")]
#[case("ab|c\ndef\nghi", "<C-v>jjhhod", "|\n\n")]
#[case("a|bcd\nefgh", "<C-v>jlOd", "a|d\neh")]
#[case("a|bcd\nefgh", "<C-v>jlOhd", "|d\nh")]
#[case("a|bcd\nefgh", "<C-v>jlOOd", "a|d\neh")]
#[case("ab|cd\nefgh", "<C-v>jhOd", "a|d\neh")]
#[case("ab|c\ndef\nghi", "<C-v>kd", "a|b\ndef\nghi")]
#[case("ab|c\ndef\nghi", "j<C-v>kld", "a|b\nde\nghi")]
// líneas cortas
#[case("ab|cd\ne\nfghi", "<C-v>jjld", "a|b\ne\nfg")]
#[case("ab|cd\ne\nfghi", "<C-v>jjly$p", "abcd|cd\ne     \nfghihi")]
#[case("ab|cd\ne\nfghi", "<C-v>jjlyP", "ab|cdcd\ne   \nfghihi")]
#[case("ab|cd\n\nfghi", "<C-v>jjld", "a|b\n\nfg")]
#[case("a|bcd\nef\nghij", "<C-v>jjlld", "|a\ne\ng")]
#[case("abc|d\nab\nabcd", "<C-v>jjd", "ab|c\nab\nabc")]
// $
#[case("a|bc\ndefgh\nij", "<C-v>j$d", "|a\nd\nij")]
#[case("a|bc\ndefgh\nij", "<C-v>jj$d", "|a\nd\ni")]
#[case("a|bc\ndefgh\nij", "<C-v>j$y$p", "abc|bc\ndefefghgh\nij")]
#[case("a|bcd\ndefgh\nghi", "<C-v>j$od", "|a\nd\nghi")]
#[case("a|bcd\ndefgh\nghi", "<C-v>j$ojd", "abcd\n|d\nghi")]
#[case("a|bcd\ndefgh\nghi", "<C-v>j$ohd", "|\n\nghi")]
#[case("a|bcd\ndefgh\nghi", "$<C-v>jd", "ab|c\ndef\nghi")]
#[case("a|bcd\ndefgh\nghi", "<C-v>j$hd", "|a\nd\nghi")]
// motions
#[case("|foo bar\nbaz qux", "<C-v>jed", "| bar\n qux")]
#[case("|foo bar\nbaz qux", "<C-v>jwd", "|ar\nux")]
#[case("foo |bar\nbaz qux", "<C-v>jbd", "|ar\nux")]
#[case("|a,b,c\nd,e,f", "<C-v>jf,;d", "|c\nf")]
#[case("|ab\ncd\nef\ngh", "<C-v>Gd", "|b\nd\nf\nh")]
#[case("ab\ncd\nef\ng|h", "<C-v>ggd", "|a\nc\ne\ng")]
#[case("ab\ncd\nef\ng|h", "<C-v>2kd", "ab\n|c\ne\ng")]
#[case("|ab\ncd\nef", "<C-v>2jld", "|\n\n")]
#[case("x|ab foo\nyab foo", "<C-v>j/foo<CR>d", "x|oo\nyoo")]
#[case("|ab\ncd", "<C-v>j0d", "|b\nd")]
#[case("  a|b\n  cd", "<C-v>j^d", " | \n  ")]
// tabs
#[case("a\tb|cd\nxyzwvutsrqp", "<C-v>jy$p", "a\tbcd|c\nxyzwvutsrqpq")]
#[case("a|b\tc\nxyzwvutsrqp", "<C-v>jllld", "a|   c\nxutsrqp")]
#[case("a|b\tc\nxyzwvutsrqp", "<C-v>jllly$p", "ab\tc|b   \nxyzwvutsryzwvqp")]
#[case("a|bcdefghij\nx\tyz", "<C-v>jlld", "|a\nx")]
#[case(
    "a|bcdefghij\nx\tyz",
    "<C-v>jllly0P",
    "|bcdefghij abcdefghij\n\tyzx\tyz"
)]
#[case("abcd|efghij\nx\tyz", "<C-v>jld", "abcd|j\nx   z")]
#[case("abcd|efghij\nx\tyz", "<C-v>jly$p", "abcdefghij|efghi\nx\tyz    y")]
#[case("abcd|efghij\nx\tyz", "<C-v>jlrx", "abcd|xxxxxj\nx   xxxxxz")]
#[case("abcd|efghij\nx\tyz", "<C-v>jlIQ<Esc>", "abcd|Qefghij\nx   Q    yz")]
#[case("abcd|efghij\nx\tyz", "<C-v>jlAQ<Esc>", "abcd|efghiQj\nx\tyQz")]
#[case("abcd|efghij\nx\tyz", "<C-v>jlcQ<Esc>", "abcd|Qj\nx   Qz")]
#[case("abcd|efghij\nx\tyz", "<C-v>jl~", "abcd|EFGHIj\nx\tYz")]
#[case("\t|ab\n\tcd", "<C-v>jld", "|\t\n\t")]
#[case("|\tab\n\tcd", "<C-v>jd", "|ab\ncd")]
#[case("|\tab\n\tcd", "<C-v>jy$p", "\tab|\t\n\tcd\t")]
// multibyte y anchos
#[case("a|ñob\ncdef", "<C-v>jld", "a|b\ncf")]
#[case("a|ñob\ncdef", "<C-v>jly$p", "añob|ño\ncdefde")]
#[case("a|ñob\ncdef", "<C-v>jlrx", "a|xxb\ncxxf")]
#[case("añ|ob\ncdef", "<C-v>jIX<Esc>", "añ|Xob\ncdXef")]
#[case("a|日本\ncdefg", "<C-v>jd", "a|本\ncfg")]
#[case("a|日本\ncdefg", "<C-v>jy$p", "a日本|日\ncdefgde")]
#[case("ab|cdefg\na日本", "<C-v>jd", "a|defg\na本")]
#[case("ab|cdefg\na日本", "<C-v>jy$p", "abcdefg|bc\na日本  日")]
#[case("ab|cdefg\na日本", "<C-v>jrx", "a|xxdefg\naxx本")]
#[case("ab|cdefg\na日本", "<C-v>jIX<Esc>", "a|Xbcdefg\naX日本")]
#[case("ab|cdefg\na日本", "<C-v>jAX<Esc>", "a|bcXdefg\na日X本")]
#[case("a|bcd\nefgh", "<C-v>jlr日", "a|日d\ne日h")]
#[case("a|bcd\nefgh", "<C-v>jllr日", "a|日 \ne日 ")]
#[case("ab|c\ndéf", "<C-v>j~", "ab|C\ndéF")]
#[case("ab|c\ndéf", "<C-v>jlU", "ab|C\ndéF")]
#[case("e\u{301}|ab\nxyz", "<C-v>jd", "e\u{301}|b\nxz")]
// c
#[case("a|bc\ndef\nghi", "<C-v>jlcX<Esc>", "a|X\ndX\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlcXY<Esc>", "aX|Y\ndXY\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlc<Esc>", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlcX<C-c>", "a|X\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>lcX<Esc>", "a|X\ndef\nghi")]
#[case("ab|c\nd\nefg", "<C-v>jjcX<Esc>", "ab|X\nd\nefX")]
#[case("ab|c\nd\nefg", "<C-v>jj$cX<Esc>", "ab|X\nd\nefX")]
#[case("a|bc\ndef\nghi", "<C-v>jlcX<BS><BS>Y<Esc>", "|Y\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlcX<CR>Y<Esc>", "aX\n|YX\nd\nghi")]
#[case("a|bc\ndef\nghi\njkl", "<C-v>jlcX<Esc>jj.", "aX\ndX\ng|X\njX")]
#[case("a|bc\ndef\nghi", "<C-v>jlcXu", "aXu|\nd\nghi")] // modo 'i'
#[case("a|bc\ndef\nghi", "<C-v>jlcX<Esc>u", "a|bc\ndef\nghi")]
// I
#[case("a|bc\ndef\nghi", "<C-v>jIX<Esc>", "a|Xbc\ndXef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jjIXY<Esc>", "a|XYbc\ndXYef\ngXYhi")]
#[case("a|bc\ndef\nghi", "<C-v>jI<Esc>", "|abc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jIX<C-c>", "a|Xbc\ndef\nghi")]
#[case("ab|c\nd\nefg", "<C-v>jjIX<Esc>", "ab|Xc\nd\nefXg")]
#[case("ab|c\nd\nefg", "<C-v>jj$IX<Esc>", "ab|Xc\nd\nefXg")]
#[case("ab|c\ndef\nghi", "<C-v>jj2IX<Esc>", "ab|XXc\ndeXXf\nghXXi")]
#[case("a|bc\ndef\nghi\njkl", "<C-v>jIX<Esc>jj.", "aXbc\ndXef\ng|Xhi\njXkl")]
#[case("a|bc\ndef\nghi\njkl", "<C-v>jIX<Esc>jj3.", "aXbc\ndXef\ng|Xhi\njXkl")]
#[case("a|bc\ndef\nghi", "<C-v>jIXY<BS>Z<Esc>", "a|XZbc\ndXZef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jIX<BS><BS><Esc>", "|bc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jIX<CR>Y<Esc>", "aX\n|Ybc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jIX<Esc>u", "a|bc\ndef\nghi")]
#[case("|\n\nabc", "<C-v>jjIX<Esc>", "|X\nX\nXabc")]
#[case("a|bc\ndef", "<C-v>jlIX<Esc>", "a|Xbc\ndXef")]
// A
#[case("a|bc\ndef\nghi", "<C-v>jAX<Esc>", "a|bXc\ndeXf\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlAX<Esc>", "a|bcX\ndefX\nghi")]
#[case("ab|cd\ne\nfghi", "<C-v>jjlAX<Esc>", "ab|cdX\ne   X\nfghiX")]
#[case("ab|cd\ne\nfghi", "<C-v>jj$AX<Esc>", "ab|cdX\neX\nfghiX")]
#[case("ab|cd\ne\nfghi", "<C-v>jjl$AX<Esc>", "ab|cdX\neX\nfghiX")]
#[case("a|b\ncdef\ng", "<C-v>jjlllAX<Esc>", "a|bX\ncdXef\ng X")]
#[case("ab\n|c\nfghi", "<C-v>jllAX<Esc>", "ab\n|c  X\nfghXi")]
#[case("a|bc\ndef\nghi", "<C-v>jA<Esc>", "a|bc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jAX<C-c>", "ab|Xc\ndef\nghi")]
#[case("a|bc\ndef\nghi\njkl", "<C-v>jAX<Esc>jj.", "abXc\ndeXf\ng|hXi\njkXl")]
#[case("a|bc\ndef\nghi\njkl", "<C-v>j$AX<Esc>jj.", "abcX\ndefX\ng|hiX\njklX")]
#[case("a|bc\ndef\nghi", "<C-v>jj2AX<Esc>", "a|bXXc\ndeXXf\nghXXi")]
#[case("|abc\ndef", "<C-v>j$AX<Esc>u", "ab|c\ndef")]
// r
#[case("a|bc\ndef\nghi", "<C-v>jlrx", "a|xx\ndxx\nghi")]
#[case("ab|cd\ne\nfghi", "<C-v>jjlrx", "ab|xx\ne\nfgxx")]
#[case("a|bc\ndefgh\nij", "<C-v>j$rx", "a|xx\ndxxxx\nij")]
#[case("a|bc\ndef\nghi\njkl", "<C-v>jlrxjj.", "axx\ndxx\ng|xx\njxx")]
#[case("a|bc\ndef\nghi", "<C-v>jlr<Esc>x", "|a\nd\nghi")]
#[case("a|bc\ndef", "<C-v>jlrxu", "a|bc\ndef")]
// mayúsculas
#[case("a|bC\ndEf\nghi", "<C-v>jl~", "a|Bc\ndeF\nghi")]
#[case("a|bC\ndEf\nghi", "<C-v>jlu", "a|bc\ndef\nghi")]
#[case("a|bC\ndEf\nghi", "<C-v>jlU", "a|BC\ndEF\nghi")]
#[case("a|bC\ndEf\nghi\njkl", "<C-v>jlUjj.", "aBC\ndEF\ng|HI\njKL")]
#[case("a|bC\ndEf", "<C-v>j$U", "a|BC\ndEF")]
// y y p
#[case("a|bc\ndef\nghi", "<C-v>jly", "a|bc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlyP", "a|bcbc\ndefef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlyp", "ab|bcc\ndeeff\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jly$p", "abc|bc\ndefef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlyjj$p", "abc\ndef\nghi|bc\n   ef")]
#[case("a|bc\ndef\nghi", "<C-v>jlyG$p", "abc\ndef\nghi|bc\n   ef")]
#[case("a|bc\ndef\nghi", "<C-v>jly3P", "a|bcbcbcbc\ndefefefef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jly2p", "ab|bcbcc\ndeefeff\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlyjjp", "abc\ndef\ngh|bci\n  ef")]
#[case("a|bc\nd\nghi", "<C-v>jjlyP", "a|bcbc\nd\nghihi")]
#[case("a|bc\nd\nghi", "<C-v>jjly$p", "abc|bc\nd  \nghihi")]
#[case("abc|\nd", "<C-v>jy$p", "abc|bc\nd  ")]
#[case("a|bc\ndef\nghi", "<C-v>jd$p", "ac|b\ndfe\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jdu", "a|bc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jdu<C-r>", "a|c\ndf\nghi")]
// registros
#[case("a|bc\ndef\nghi", "\"a<C-v>jly\"ap", "a|bc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl\"ay\"aP", "a|bcbc\ndefef\nghi")]
#[case(
    "a|bc\ndef\nghi",
    "<C-v>jl\"ayj<C-v>\"Ay\"ap",
    "abc\nde|bcf\nghefi\n  e"
)]
#[case("a|bc\ndef\nghi", "<C-v>jl\"ayyy\"Ap", "ab|bcc\ndeeff\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jd\"1p", "ac|b\ndfe\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>ld$\"-p", "a|bc\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jd\"-p", "a|c\ndf\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl\"_dP", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jly\"0P", "a|bcbc\ndefef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl\"ad.", "|\n\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl\".d", "a|bc\ndef\nghi")]
#[case("a|bc\ndef", "qa<C-v>jIX<Esc>q", "a|Xbc\ndXef")]
#[case(
    "a|bc\ndef\nghi\njkl",
    "qa<C-v>jIX<Esc>jjq@a",
    "aXbc\ndXef\ngXhi\nj|Xkl"
)]
#[case(
    "a|bc\ndef\nghi\njkl",
    "qa<C-v>jIX<Esc>jjqu@a",
    "aXbc\ndXef\ng|hi\njkl"
)]
#[case("a|bc\ndef\nghi", "\"ayy<C-v>jl\"ap", "a\nd\n|abc\nghi")]
// p sobre un bloque
#[case("|foo\nabcd\nefgh\nijkl", "yiwj<C-v>jjlp", "foo\nfo|ocd\nfoogh\nfookl")]
#[case("|foo\nabcd\nefgh\nijkl", "yiwj<C-v>jjlP", "foo\nfo|ocd\nfoogh\nfookl")]
#[case(
    "|foo\nabcd\nefgh\nijkl",
    "yiwj<C-v>jjlp0P",
    "foo\n|abfoocd\neffoogh\nijfookl"
)]
#[case("|foo\nabcd\nefgh\nijkl", "yiwjl<C-v>jj$p", "foo\nafo|o\nefoo\nifoo")]
#[case("|foo\nabcd\nef\nijkl", "yiwjll<C-v>jjlp", "foo\nabfo|o\neffoo\nijfoo")]
#[case(
    "|foo\nabcd\nefg\nijkl",
    "yiwjll<C-v>jjlp",
    "foo\nabfo|o\neffoo\nijfoo"
)]
#[case(
    "|foo\nabcd\ne\tf\nijkl",
    "yiwjll<C-v>jjlp",
    "foo\nabfo|o\ne foo    f\nijfoo"
)]
#[case(
    "|foo\nabcd\nefgh\nijkl",
    "yiwj<C-v>jjl2p",
    "foo\nfoofo|ocd\nfoofoogh\nfoofookl"
)]
#[case("|foo\nabcd\nefgh\nijkl", "yiwj<C-v>jjlp.", "foo\nfo|d\nfoh\nfol")]
#[case("|foo\nabcd\nefgh\nijkl", "yiwj<C-v>jjlPj.", "foo\nfoocd\nfo|h\nfol")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjl<C-v>jlp", "xy\nzw\na|xyd\nezwh")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjl<C-v>jlP", "xy\nzw\na|xyd\nezwh")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjll<C-v>jlp", "xy\nzw\nab|xy\nefzw")]
#[case("|xy\nzw\nabcd\nefgh", "yyjjl<C-v>jlp", "xy\nzw\nad\neh\n|xy")]
#[case("|xy\nzw\nabcd\nefgh", "yyjjl<C-v>jlP", "xy\nzw\n|xy\nad\neh")]
#[case("|xy\nzw\nabcd\nefgh", "yyjjjl<C-v>klp", "xy\nzw\nad\n|xy\neh")]
#[case("|xy\nzw\nabcd\nefgh", "vjyjjl<C-v>jlp", "xy\nzw\na|xy\nzd\neh")]
#[case("|xy\nzw\nabcd\nefgh", "vjyjjll<C-v>jlp", "xy\nzw\nab|xy\nz\nef")]
#[case("|xy\nzw\nabcd\nefgh", "yiwjj<C-v>jlp\"-p", "xy\nzw\nx|ycd\nxygh")]
#[case("|xy\nzw\nabcd\nefgh", "yiwjj<C-v>jlp\"1p", "xy\nzw\nxy|abcd\nxyefgh")]
#[case("a|bcd\nefgh", "<C-v>jl\"_p", "a|d\neh")]
// v y V con un registro por bloque
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjlvp", "xy\nzw\na|xycd\nezwfgh")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjlvlp", "xy\nzw\na|xyd\nezwfgh")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjlv$p", "xy\nzw\na|xyefgh\n zw")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjVp", "xy\nzw\n|xy\nzw\nefgh")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjVjp", "xy\nzw\n|xy\nzw")]
#[case("|xy\nzw\nabcd\nefgh", "<C-v>jlyjjlvP", "xy\nzw\na|xycd\nezwfgh")]
// gv y [count]ctrl-v
#[case("a|bc\ndef\nghi", "<C-v>jl<Esc>0gvd", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>j$<Esc>0gvd", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl<Esc>gvo<Esc>x", "a|c\ndef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jl<Esc>vgvd", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlv<Esc>gvd", "|a\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jldgvd", "|a\nd\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlyvgvd", "|a\nd\nghi")]
#[case("a|bcd\ndefg\nghij\nklmn", "<C-v>jly1<C-v>d", "a|d\ndg\nghij\nklmn")]
#[case("a|bcd\ndefg\nghij\nklmn", "<C-v>jlyj2<C-v>d", "abcd\n|d\ng\nk")]
#[case("a|bcd\ndefg\nghij\nklmn", "<C-v>j$y1<C-v>d", "|a\nd\nghij\nklmn")]
#[case("a|bcd\ndefg\nghij\nklmn", "<C-v>jly1vd", "a|d\ndg\nghij\nklmn")]
#[case("a|bcd\ndefg\nghij\nklmn", "vly1<C-v>d", "a|d\ndefg\nghij\nklmn")]
#[case("|abcdef", "3<C-v>d", "|def")]
// .
#[case("a|bcd\nefgh\nijkl\nmnop", "<C-v>jldjj.", "ad\neh\ni|l\nmp")]
#[case("a|bcd\nefgh\nijkl\nmnop", "<C-v>jldjj0.", "ad\neh\n|kl\nop")]
#[case("a|bcd\nefgh\nijkl\nmnop", "<C-v>j$djj.", "a\ne\n|\n")]
#[case("a|bcd\nefgh\nijkl\nmnop", "<C-v>jld3.", "|a\ne\nijkl\nmnop")]
#[case("a|bcd\nefgh\nijkl\nmnop", "<C-v>jldu.", "a|d\neh\nijkl\nmnop")]
// p de varias líneas por caracteres o por líneas: el lado lo decide dónde
// quedó el cursor al borrar el bloque
#[case("|X\nYz\nabc\nabc", "vjyjjll<C-v>jp", "X\nYz\nab|X\nY\nab")]
#[case("|X\nYz\nabc\nabc", "vjyjjl<C-v>jp", "X\nYz\na|X\nYc\nac")]
#[case("|X\nYz\nabc\nabc", "Vyjjll<C-v>jP", "X\nYz\nab\n|X\nab")]
#[case("|X\nYz\nabc\nabc", "Vyjjl<C-v>jP", "X\nYz\n|X\nac\nac")]
// > y <
#[case("ab|cd\nefgh", "<C-v>j>", "ab|\t  cd\nef\t  gh")]
#[case("ab|cd\nefgh", "<C-v>j2>", "ab|\t\t  cd\nef\t\t  gh")]
#[case("ab|cd\nefgh\nij", "<C-v>jj>", "ab|\t  cd\nef\t  gh\nij\t  ")]
#[case("ab|cd\ne\nfghi", "<C-v>jj>", "ab|\t  cd\ne\nfg\t  hi")]
#[case("ab|cd\n\nefgh", "<C-v>jj>", "ab|\t  cd\n\nef\t  gh")]
#[case("ab|cd\nefgh", "<C-v>j$>", "ab|\t  cd\nef\t  gh")]
#[case("a  |b\nc  d", "<C-v>j>", "a\t |  b\nc\t   d")]
#[case("a\t|b\ncdefghij", "<C-v>j>", "a\t|\tb\ncdefghij\t")]
#[case(
    "ab\tc|d\nefghijklmnop",
    "<C-v>jhhh>",
    "ab|\t\tcd\nefghij\t      klmnop"
)]
#[case(
    "a|b\t\tcd\nxyzwvutsrqponm",
    "<C-v>jll>",
    "a|\t b\t\tcd\nx\t yzwvutsrqponm"
)]
#[case("añ|bc\nañbc", "<C-v>j>", "añ|\t  bc\nañ\t  bc")]
#[case("a日|bc\nxyzw", "<C-v>j>", "a日|\t   bc\nxyz\t   w")]
#[case("ab|\t\tcd\nef\t\tgh", "<C-v>j<lt>", "ab|\tcd\nef\tgh")]
#[case("ab|\t\tcd\nef\t\tgh", "<C-v>j3<lt>", "ab|cd\nefgh")]
#[case("ab   |    cd\nef        gh", "<C-v>j<lt>", "ab   |cd\nef   gh")]
#[case("ab|cd\nef\t\tgh", "<C-v>j<lt>", "ab|cd\nef\tgh")]
#[case(
    "a|b\t\tcd\nxyzwvutsrqponm",
    "<C-v>jll<lt>",
    "a|b\t\tcd\nxyzwvutsrqponm"
)]
#[case(
    "ab|cd\nefgh\nijkl\nmnop",
    "<C-v>j>jj.",
    "ab\t  cd\nef\t  gh\nijk|\t   l\nmno\t   p"
)]
#[case(
    "ab|cd\nefgh\nijkl\nmnop",
    "<C-v>j2>jj3.",
    "ab\t\t  cd\nef\t\t  gh\nijk|\t\t   l\nmno\t\t   p"
)]
#[case("ab|cd\nefgh\nijkl\nmnop", "<C-v>j>u", "ab|cd\nefgh\nijkl\nmnop")]
#[case(
    "ab|\t\tcd\nef\t\tgh\nij\t\tkl",
    "<C-v>j<lt>j.",
    "ab\tcd\nef|gh\nij\tkl"
)]
// tras o, un $ no sigue valiendo para la selección siguiente
#[case("a|bc\ndefgh\nijklmn", "<C-v>j$o<Esc>$<C-v>jjd", "a|b\nde\nij")]
#[case("abcd|ef\na\tb", "<C-v>j$oOj<Esc>", "abcdef\na\t|b")]
#[case("abcd|ef\na\tb", "<C-v>j$oOjd", "abcdef\n|a")]
// clic en Insert, tabs escritos y r<Tab>
#[case(
    "a|bc\ndef\nghi",
    "<C-v>jIX<LeftMouse><0,2><LeftRelease><0,2>Y<Esc>",
    "aXbc\ndef\n|Yghi"
)]
#[case("a|bc\ndef\nghi", "<C-v>jI<Tab><Esc>", "a|\tbc\nd\tef\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlr<Tab>", "a|\t\t\nd\t\t\nghi")]
#[case("a|bc\ndef\nghi", "<C-v>jlA<Tab>X<Esc>", "a|bc\tX\ndef\tX\nghi")]
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
#[case("|abc\ndef", "<C-v>", "|abc\ndef")]
#[case("|abc\ndef", "<C-v>jl", "¦abc\nd|ef")]
#[case("a|bc\ndef", "<C-v>jlo", "a|bc\nde¦f")]
#[case("a|bcd\nefgh", "<C-v>jlO", "ab¦cd\ne|fgh")]
#[case("a|bcd\nefgh", "<C-v>jlOO", "a¦bcd\nef|gh")]
#[case("ab|cd\nefgh", "<C-v>jhO", "a¦bcd\nef|gh")]
#[case("ab|cd\nefgh", "<C-v>jhOO", "ab¦cd\ne|fgh")]
#[case("a|bcd\nefgh", "<C-v>jlOj", "ab¦cd\ne|fgh")]
#[case("a|bc\ndefgh", "<C-v>j$", "a¦bc\ndefgh|")]
#[case("a|bc\ndefgh", "<C-v>j$o", "a|bc\ndefgh¦")]
#[case("a|bcd\ndefgh\nghi", "<C-v>j$oj", "abcd\nd|efgh¦\nghi")]
#[case("a|bcd\ndefgh\nghi", "<C-v>j$O", "abcd¦\nd|efgh\nghi")]
#[case("|abc\ndef", "v<C-v>", "|abc\ndef")]
#[case("|abc\ndef", "vj<C-v>", "¦abc\n|def")]
#[case("|abc\ndef", "Vj<C-v>", "¦abc\n|def")]
#[case("|abc\ndef", "<C-v>jv", "¦abc\n|def")]
#[case("|abc\ndef", "<C-v>jV", "¦abc\n|def")]
#[case("ab|cd\ne\nfghi", "<C-v>jj", "ab¦cd\ne\nfg|hi")]
#[case("ab|cd\ne\nfghi", "<C-v>jjl", "ab¦cd\ne\nfgh|i")]
#[case("ab|cd\ne\nfghi", "<C-v>jjlO", "abc¦d\ne\nfg|hi")]
#[case("foo b|ar\nbaz qux", "<C-v>iw", "foo ¦ba|r\nbaz qux")]
#[case("foo b|ar\nbaz qux", "<C-v>jiw", "foo b¦ar\nbaz qu|x")]
#[case("foo b|ar\nbaz qux", "<C-v>aw", "foo¦ ba|r\nbaz qux")]
#[case("x \"a|b\" y", "<C-v>i\"", "x \"¦a|b\" y")]
#[case("f(a|b)", "<C-v>i(", "f(¦a|b)")]
#[case("a\nb|b\n\nc", "<C-v>ip", "¦a\n|bb\n\nc")]
#[case("a|bc\ndef", "<C-v>jl<Esc>gv", "a¦bc\nde|f")]
#[case("a|bc\ndef", "<C-v>j$<Esc>gv", "a¦bc\ndef|")]
#[case("a|bc\ndef", "<C-v>jl<Esc>vgv", "a¦bc\nde|f")]
#[case("a|bc\ndef", "<C-v>jl<Esc>vgvgv", "abc\nde|f")]
#[case(
    "a|bcd\ndefg\nghij\nklmn",
    "<C-v>jly1<C-v>",
    "a¦bcd\nde|fg\nghij\nklmn"
)]
#[case(
    "a|bcd\ndefg\nghij\nklmn",
    "<C-v>jly2<C-v>",
    "a¦bcd\ndefg\nghij\nklmn|"
)]
#[case(
    "a|bcd\ndefg\nghij\nklmn",
    "<C-v>j$y1<C-v>",
    "a¦bcd\ndefg|\nghij\nklmn"
)]
#[case("a|bcd\ndefg\nghij\nklmn", "<C-v>jly1v", "a¦bcd\nde|fg\nghij\nklmn")]
#[case("a|bcd\ndefg\nghij\nklmn", "vly1<C-v>", "a¦b|cd\ndefg\nghij\nklmn")]
#[case("a|bcd\ndefg\nghij\nklmn", "Vjy1<C-v>", "¦abcd\n|defg\nghij\nklmn")]
#[case("|abcdef", "3<C-v>", "¦ab|cdef")]
#[case("|abc\ndef", "<C-v>jr<Esc>", "¦abc\n|def")]
#[case("x|ab foo\nyab foo", "<C-v>j/foo<CR>", "x¦ab foo\nyab |foo")]
#[case("a|b\tc\nxyzwvutsrqp", "<C-v>jlllO", "ab¦\tc\nx|yzwvutsrqp")]
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

/// nvim no tiene multi-cursor: estos casos solo prueban el motor. Cada
/// selección es su propio bloque.
#[rstest]
#[case("|abc\nabc\n|abc\nabc", "<C-v>jld", "|c\nc\n|c\nc")]
#[case("|abc\nabc\n|abc\nabc", "<C-v>jlIX<Esc>", "|Xabc\nXabc\n|Xabc\nXabc")]
#[case("|abc\nabc\n|abc\nabc", "<C-v>jAX<Esc>", "|aXbc\naXbc\n|aXbc\naXbc")]
#[case("|abc\nabc\n|abc\nabc", "<C-v>jcX<Esc>", "|Xbc\nXbc\n|Xbc\nXbc")]
#[case("|abc\nabc\n|abc\nabc", "<C-v>jrx", "|xbc\nxbc\n|xbc\nxbc")]
// cada bloque pega del lado que le toca: el primero llega al fin de línea
// y el segundo tiene texto después
#[case(
    "|x\n-\nabc\nabc\n|y\n-\nabcd\nabcd",
    "yljjll<C-v>jp",
    "x\n-\nab|x\ny\nab\ny\n-\nab|x\nyd\nabd"
)]
#[case(
    "|x\n-\nabc\nabc\n|y\n-\nabcd\nabcd",
    "Vyjjll<C-v>jP",
    "x\n-\nab\n|x\ny\nab\ny\n-\n|x\ny\nabd\nabd"
)]
// dos bloques en las mismas líneas se solapan en el texto y se funden
#[case("|abc |abc\nabc abc", "<C-v>jd", "|bc\nbc")]
fn multiple_cursors(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(support::run_engine(before, keys).text, after);
}

#[test]
fn reports_the_block_mode() {
    use metalmind_vim::{Key, Mode, Vim, VisualKind};

    let (mut buffer, mut selections) = support::parse("|ab\ncd");
    let mut vim = Vim::new();
    let mut feed = |vim: &mut Vim, key| vim.feed(key, &mut buffer, &mut selections).unwrap();
    feed(&mut vim, Key::Ctrl('v'));
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Block));
    feed(&mut vim, Key::Char('v'));
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Char));
    feed(&mut vim, Key::Ctrl('v'));
    assert_eq!(vim.mode(), Mode::Visual(VisualKind::Block));
    feed(&mut vim, Key::Ctrl('v'));
    assert_eq!(vim.mode(), Mode::Normal);
}

#[test]
fn block_actions_run_by_name() {
    use metalmind_vim::{Action, Vim};

    let (mut buffer, mut selections) = support::parse("|ab\ncd");
    let mut vim = Vim::new();
    let names = [
        "vim::toggle_visual_block",
        "vim::down",
        "vim::append_to_block",
    ];
    for name in names {
        let action = Action::from_name(name).unwrap();
        vim.execute(action, &mut buffer, &mut selections).unwrap();
    }
    vim.paste_text("X", &mut buffer, &mut selections).unwrap();
    let escape = Action::from_name("vim::normal_mode").unwrap();
    vim.execute(escape, &mut buffer, &mut selections).unwrap();
    assert_eq!(buffer.to_string(), "aXb\ncXd");
}

/// Un clic termina la selección como `<Esc>`: el `$` que `o` dejó valiendo
/// para operar no pasa a la selección siguiente.
#[test]
fn a_click_ends_the_line_end_left_by_o() {
    use metalmind_buffer::{ByteOffset, Selection, Selections};
    use metalmind_vim::Vim;

    let (mut buffer, mut selections) = support::parse("a|bc\ndefgh\nijklmn");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "<C-v>j$o");
    vim.cursor_moved();
    selections = Selections::single(Selection::cursor(ByteOffset::new(2)));
    support::feed(&mut vim, &mut buffer, &mut selections, "$<C-v>jjd");
    assert_eq!(buffer.to_string(), "ab\nde\nij");
}

/// El `tabstop` es el de la ventana que presta quien aloja el motor.
#[test]
fn the_block_measures_tabs_with_the_viewport_tabstop() {
    use metalmind_vim::{Viewport, Vim};

    let (mut buffer, mut selections) = support::parse("a\tb|c\nxyzwvutsrqp");
    let mut vim = Vim::new();
    let mut viewport = Viewport::new(10);
    viewport.set_tabstop(4);
    vim.swap_viewport(&mut viewport);
    support::feed(&mut vim, &mut buffer, &mut selections, "<C-v>jd");
    assert_eq!(buffer.to_string(), "a\tb\nxyzwvtsrqp");
}

/// Lo que la UI necesita para pintar el bloque.
#[test]
fn the_visual_block_tells_what_to_paint() {
    use metalmind_buffer::ByteOffset;
    use metalmind_vim::{Viewport, Vim, VisualBlock};

    let (mut buffer, mut selections) = support::parse("|abcd\na\tb\nxy");
    let mut vim = Vim::new();
    support::feed(&mut vim, &mut buffer, &mut selections, "<C-v>jjll");
    assert_eq!(
        support::render(&buffer, [selections.primary().head]),
        "abcd\na\tb\nxy|"
    );
    let block = VisualBlock::new(&buffer, selections.primary(), &Viewport::default());
    assert_eq!(block.lines(), 0..=2);
    assert_eq!(block.columns(), 0..=2);
    assert!(!block.to_line_end());
    let bytes = |line| {
        let range = block.bytes_on(&buffer, line);
        let start = buffer.line_start(line);
        range.start.get() - start.get()..range.end.get() - start.get()
    };
    assert_eq!([bytes(0), bytes(1), bytes(2)], [0..3, 0..2, 0..2]);
    assert_eq!(
        block.bytes_on(&buffer, 2).end,
        ByteOffset::new(buffer.len().get())
    );
}
