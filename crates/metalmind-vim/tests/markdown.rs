#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! La edición de markdown del motor. Donde nvim con `ft=markdown` hace lo
//! mismo (`>`, `<`, `]]` y `[[` en Normal), cada caso corre también en él;
//! el resto (listas, checkboxes, secciones) no tiene equivalente y solo
//! prueba el motor.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use std::num::NonZeroUsize;

use metalmind_buffer::{Buffer, Selections};
use metalmind_vim::{Action, Indent, Layout, Markdown, Viewport, Vim};
use pretty_assertions::assert_eq;
use rstest::rstest;
use rstest_reuse::{self, apply, template};

/// `(antes, teclas, después)`, comparados con nvim 0.12.6 con
/// `ft=markdown` (`sw=4 ts=4 et`, y `]]`/`[[` de su ftplugin).
#[template]
#[rstest]
// >> << con count, motions y text objects
#[case("  abc d|ef\nx", ">>", "      a|bc def\nx")]
#[case("  abc d|ef\nx", "<lt><lt>", "abc de|f\nx")]
#[case("ab|c\nx", ">>", "  |  abc\nx")]
#[case("   a|bc\nx", "<lt><lt>", "ab|c\nx")]
#[case("|abc\nx", "<lt><lt>", "|abc\nx")]
#[case("ab|c\n\nx", "3>>", "  |  abc\n\n    x")]
#[case("ab|c\nx\ny", ">j", "  |  abc\n    x\ny")]
#[case("ab|c\nx\ny", ">>j.", "    abc\n  |  x\ny")]
#[case("ab|c\nx\ny", ">>.", "  |      abc\nx\ny")]
#[case("ab|c\nx\ny", ">>u", "ab|c\nx\ny")]
#[case("a\nb|c\nd\n\ne", ">ip", " |   a\n    bc\n    d\n\ne")]
#[case("ab|c\nx\ny", ">G", "  |  abc\n    x\n    y")]
#[case("a\n  |b", ">k", "  |  a\n      b")]
#[case("        a|b", "<lt><lt>", "    a|b")]
#[case("\tab|c", "<lt><lt>", "ab|c")]
#[case("|  ", ">>", "|      ")]
#[case("ab|c\nx\ny", "Vj>", "  |  abc\n    x\ny")]
#[case("ab|c\nx\ny", "Vj2>", "  |      abc\n        x\ny")]
#[case("ab|c\nx\ny", "Vj>.", "  |      abc\n        x\ny")]
#[case("        ab|c", "v<lt>", "    ab|c")]
// ]] [[: al inicio de la línea del heading siguiente o anterior
#[case("|a\n# h1\nb\n## h2\nc", "]]", "a\n|# h1\nb\n## h2\nc")]
#[case("a\n# h1\nb\n## h2\n|c", "[[", "a\n# h1\nb\n|## h2\nc")]
#[case("a\n# h1\nb\n## h|2\nc", "[[", "a\n|# h1\nb\n## h2\nc")]
#[case("a\n# h|1\nb\n## h2\nc", "]]", "a\n# h1\nb\n|## h2\nc")]
#[case("a\n# h1\nb\n|## h2\nc", "]]", "a\n# h1\nb\n|## h2\nc")]
#[case("a|b\n# h1", "[[", "a|b\n# h1")]
#[case("|a\n####### no\n###### h6", "]]", "a\n####### no\n|###### h6")]
#[case("|a\n```\n# no\n```\n# si", "]]", "a\n```\n# no\n```\n|# si")]
#[case("|a\n   # ind", "]]", "a\n|   # ind")]
#[case("|a\n#no", "]]", "|a\n#no")]
#[case("x|y\n# h1\nb", "]]j", "xy\n# h1\n|b")]
fn shifts_and_headings(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {}

#[apply(shifts_and_headings)]
fn engine(
    #[case] before: &str,
    #[case] keys: &str,
    #[case] after: &str,
    #[values(false, true)] layout: bool,
) {
    assert_eq!(run(before, keys, Markdown::default(), layout).0, after);
}

#[apply(shifts_and_headings)]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn nvim_markdown_oracle(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let nvim = support::nvim::run_after("set filetype=markdown", before, keys);
    assert_eq!(nvim.text, after, "la tabla no coincide con nvim");
    let (_, mode) = run(before, keys, Markdown::default(), false);
    assert_eq!(nvim.mode, mode);
}

/// Lo que nvim no hace con `ft=markdown`: continuar listas, checkboxes,
/// secciones, `]]` con count o con operador, y `>>` con los hijos de un ítem.
#[rstest]
// Enter en Insert continúa la lista
#[case("- fo|o", "A<CR>", "- foo\n- |")]
#[case("- fo|o", "i<CR>", "- fo\n- |o")]
#[case("* a|", "a<CR>b<Esc>", "* a\n* |b")]
#[case("+   a|", "a<CR>", "+   a\n+   |")]
#[case("  - a|", "a<CR>", "  - a\n  - |")]
#[case("1. a|", "a<CR>", "1. a\n2. |")]
#[case("9) a|", "a<CR>", "9) a\n10) |")]
#[case("- [x] a|", "a<CR>", "- [x] a\n- [ ] |")]
#[case("- [ ] a|", "a<CR>", "- [ ] a\n- [ ] |")]
#[case("1. [x] a|", "a<CR>", "1. [x] a\n2. [ ] |")]
#[case("- a|", "a<CR><CR>", "- a\n|")]
#[case("- a|", "a<CR><CR><CR>", "- a\n\n|")]
#[case("- [ ] a|", "a<CR><CR>", "- [ ] a\n|")]
#[case("  - a|", "a<CR><CR>", "  - a\n|")]
#[case("1. a|", "a<CR><CR>x", "1. a\nx|")]
#[case("|- a", "i<CR>", "\n|- a")]
#[case("-| a", "i<CR>", "-\n| a")]
#[case("a|", "a<CR>", "a\n|")]
#[case("-a|", "a<CR>", "-a\n|")]
#[case("```\n- a|\n```", "a<CR>", "```\n- a\n|\n```")]
// y renumera los ítems que siguen
#[case("1. a|\n2. b\n3. c", "a<CR>x<Esc>", "1. a\n2. |x\n3. b\n4. c")]
#[case(
    "1. a|\n2. b\n   - c\n3. d\n\n4. e",
    "a<CR>",
    "1. a\n2. |\n3. b\n   - c\n4. d\n\n4. e"
)]
#[case("1. a\n   1. b|\n2. c", "a<CR>", "1. a\n   1. b\n   2. |\n2. c")]
#[case("1. a|\n2. b\n3. c", "a<CR><CR>", "1. a\n|\n2. b\n3. c")]
// el count, `.` y `u` lo repiten o deshacen entero
#[case("- a|", "3ax<CR><Esc>", "- ax\n- x\n- x\n-| ")]
#[case("1. a|\n2. b", "a<CR>x<Esc>j.", "1. a\n2. x\n3. b\n4. |x")]
#[case("1. a|\n2. b", "a<CR>x<Esc>u", "1. |a\n2. b")]
// lo pegado no continúa la lista
#[case("- a|", "a<PasteStart>\nb<PasteEnd><Esc>", "- a\n|b")]
// o y O
#[case("- |a", "o", "- a\n- |")]
#[case("- |a", "O", "- |\n- a")]
#[case("- [x] |a", "o", "- [x] a\n- [ ] |")]
#[case("1. |a\n2. b", "o", "1. a\n2. |\n3. b")]
#[case("1. a\n2. |b", "O", "1. a\n2. |\n3. b")]
#[case("1. |a", "Ox<Esc>", "1. |x\n2. a")]
#[case("|a", "o", "a\n|")]
#[case("```\n- |a\n```", "o", "```\n- a\n|\n```")]
#[case("- |a", "3ox<Esc>", "- a\n- x\n- x\n- |x")]
#[case("- |a", "3o<Esc>", "- a\n- \n- \n-| ")]
#[case("1. |a\n2. b", "2ox<Esc>", "1. a\n2. x\n3. |x\n4. b")]
#[case("- |a", "ox<Esc>.", "- a\n- x\n- |x")]
#[case("- |a", "o<Esc>u", "- |a")]
// ]] [[ con count, en Visual y con operador
#[case("|a\n# 1\n# 2\n# 3", "2]]", "a\n# 1\n|# 2\n# 3")]
#[case("|a\n# 1\n# 2", "9]]", "a\n# 1\n|# 2")]
#[case("# 1\n# 2\n# |3", "2[[", "|# 1\n# 2\n# 3")]
#[case("|a\nb\n# h", "d]]", "|# h")]
#[case("a|b\nc\n# h", "d]]", "|a\n# h")]
#[case("|a\nb\n# h", "v]]d", "| h")]
#[case("|a\nb", "]]", "|a\nb")]
// los links no son headings, y `[[` en Insert se escribe
#[case("|a [[b]]\n[[#c]]\n# h", "]]", "a [[b]]\n[[#c]]\n|# h")]
#[case("# h\n[[b]]\n|x", "[[", "|# h\n[[b]]\nx")]
#[case("|", "i[[b]]<Esc>", "[[b]|]")]
// el frontmatter no tiene headings
#[case("|---\n# no: x\n---\n# si", "]]", "---\n# no: x\n---\n|# si")]
// ih ah: la sección, hasta el siguiente heading de su nivel o mayor
#[case("# a\n|x\n## b\ny\n# c\nz", "dah", "|# c\nz")]
#[case("# a\nx\n## b\n|y\n# c\nz", "dah", "# a\nx\n|# c\nz")]
#[case("# a\nx\n## b\n|y\n# c\nz", "d2ah", "|# c\nz")]
#[case("# a\nx\n## b\n|y\n# c\nz", "d3ah", "# a\nx\n## b\n|y\n# c\nz")]
#[case("# |a\n\nx\ny\n\n# c", "dih", "# a\n\n|\n# c")]
#[case("# |a\n\nx\n\n# c", "cihz<Esc>", "# a\n\n|z\n\n# c")]
#[case("# |a\n\n# c", "dih", "# |a\n\n# c")]
#[case("|x\n# a", "dah", "|x\n# a")]
#[case("# a\n|x", "yahP", "|# a\nx\n# a\nx")]
#[case("# a\n## b\n|y\n# c", "vahd", "# a\n|# c")]
#[case("# a\n## b\n|y\n# c", "vahahd", "|# c")]
#[case("# a\n```\n# no\n```\n|x\n# c", "dah", "|# c")]
#[case("# a\n|x\n# b\ny", ">ah", "|    # a\n    x\n# b\ny")]
// >> << con los hijos de un ítem
#[case(
    "- |a\n  - b\n    c\n\n  d\n- e",
    ">>",
    "  |  - a\n      - b\n        c\n\n      d\n- e"
)]
#[case("    - |a\n      - b\n- c", "<lt><lt>", "- |a\n  - b\n- c")]
#[case("  - |a\n    - b", "<lt><lt>", "- |a\n  - b")]
#[case("- |a\n  - b", "<lt><lt>", "- |a\n  - b")]
#[case("- |a\n  - b\n\n- c", ">>", "  |  - a\n      - b\n\n- c")]
#[case("- |a\n- b\n  - c", "2>>", "  |  - a\n    - b\n      - c")]
#[case("- |a\n  - b\n- c", ">j", "  |  - a\n      - b\n- c")]
#[case("- |a\n  - b\n- c", ">>j.", "    - a\n  |        - b\n- c")]
// En Visual las líneas son las elegidas: `<` es el de vim, sin hijos.
#[case("  - |a\n    - b", "Vj<lt>", "- |a\n- b")]
// checkboxes
#[case("- [ ] |a", "<toggle>", "- [x] |a")]
#[case("- [x] |a", "<toggle>", "- [ ] |a")]
#[case("- [X] |a", "<toggle>", "- [ ] |a")]
#[case("- |a", "<toggle>", "- |[ ] a")]
#[case("1. a|b", "<toggle>", "1. [ ] a|b")]
#[case("|a", "<toggle>", "|a")]
#[case("```\n|- [ ] a\n```", "<toggle>", "```\n|- [ ] a\n```")]
#[case("|- [ ] a\n- [x] b\n- c", "3<toggle>", "|- [x] a\n- [ ] b\n- [ ] c")]
#[case("|- [ ] a\n- [ ] b", "<toggle>j.", "- [x] a\n|- [x] b")]
#[case("|- [ ] a\n- [ ] b", "<toggle>u", "|- [ ] a\n- [ ] b")]
#[case("- [ ] a\n- [ ] |b\nc", "Vk<toggle>", "- [x] |a\n- [x] b\nc")]
fn markdown_editing(
    #[case] before: &str,
    #[case] keys: &str,
    #[case] after: &str,
    #[values(false, true)] layout: bool,
) {
    assert_eq!(run(before, keys, Markdown::default(), layout).0, after);
}

/// Nota no tiene usuarios que esperen `>>` de vim en una lista, pero quien
/// lo apaga lo tiene.
#[rstest]
#[case("- a|", "a<CR>", "- a\n|")]
#[case("- |a", "o", "- a\n|")]
#[case("- |a\n  - b", ">>", "  |  - a\n  - b")]
#[case("    - |a\n      - b", "<lt><lt>", "- |a\n      - b")]
fn markdown_editing_turned_off(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let off = Markdown {
        continue_lists: false,
        indent_list_children: false,
    };
    assert_eq!(run(before, keys, off, false).0, after);
}

/// `>`, `<`, `:>` y `>` en un bloque con otro `shiftwidth` (y el mismo
/// `tabstop`), con tabs (`noexpandtab`) o sin ellos.
#[rstest]
#[case("|- a\n  - b", 2, true, ">>", "|  - a\n    - b")]
#[case("|a", 4, false, ">>", "|\ta")]
#[case("  |a", 4, false, ">>", "|\t  a")]
#[case("\t\t|a", 8, true, "<lt><lt>", "        |a")]
#[case("ab|c\nx", 2, false, "Vj3>", "\t|\t\tabc\n\t\t\tx")]
#[case("|abc\n  x", 4, true, "<C-v>j>", "|    abc\n      x")]
#[case("abc\nx|", 3, false, ":%>><CR>", "\t\tabc\n\t\t|x")]
fn indent_width_and_tabs(
    #[case] before: &str,
    #[case] shiftwidth: usize,
    #[case] expandtab: bool,
    #[case] keys: &str,
    #[case] after: &str,
) {
    let (mut buffer, mut selections) = support::parse(before);
    let mut vim = Vim::new();
    vim.set_indent(Indent {
        shiftwidth: NonZeroUsize::new(shiftwidth).unwrap(),
        expandtab,
    });
    let mut viewport = Viewport::default();
    viewport.set_tabstop(shiftwidth);
    vim.swap_viewport(&mut viewport);
    support::feed(&mut vim, &mut buffer, &mut selections, keys);
    assert_eq!(support::render(&buffer, heads(&selections)), after);
}

/// Los de arriba sin el ítem con hijos, que nvim no lleva consigo.
#[rstest]
#[case("|a", 4, false, ">>", "|\ta")]
#[case("  |a", 4, false, ">>", "|\t  a")]
#[case("\t\t|a", 8, true, "<lt><lt>", "        |a")]
#[case("ab|c\nx", 2, false, "Vj3>", "\t|\t\tabc\n\t\t\tx")]
#[case("|abc\n  x", 4, true, "<C-v>j>", "|    abc\n      x")]
#[case("abc\nx|", 3, false, ":%>><CR>", "\t\tabc\n\t\t|x")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn indent_width_and_tabs_like_nvim(
    #[case] before: &str,
    #[case] shiftwidth: usize,
    #[case] expandtab: bool,
    #[case] keys: &str,
    #[case] after: &str,
) {
    let et = if expandtab { "et" } else { "noet" };
    let setup = format!("setlocal sw={shiftwidth} ts={shiftwidth} {et}");
    assert_eq!(support::nvim::run_after(&setup, before, keys).text, after);
}

/// nvim no tiene multi-cursor.
#[rstest]
#[case("1. a|\n2. b|\n3. c", "a<CR>", "1. a\n2. |\n3. b\n4. |\n5. c")]
#[case("- a|\n- b|", "a<CR><CR>", "- a\n|\n- b\n|")]
#[case("|- [ ] a\n|- [x] b", "<toggle>", "|- [x] a\n|- [ ] b")]
#[case("|# a\nx\n|# b\ny", "dah", "|")]
fn multiple_cursors(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    assert_eq!(run(before, keys, Markdown::default(), true).0, after);
}

#[test]
fn every_markdown_action_has_a_name() {
    for name in [
        "vim::indent",
        "vim::outdent",
        "vim::next_heading",
        "vim::previous_heading",
        "vim::inner_section",
        "vim::a_section",
        "editor::toggle_checkbox",
    ] {
        assert!(Action::from_name(name).is_some(), "{name}");
    }
}

/// Como `support::feed`, con `<toggle>` como `editor::toggle_checkbox`, que
/// no tiene tecla en el motor, y el motor como lo deja la app en una nota:
/// lo que pone el ftplugin de markdown de nvim (`sw=4 ts=4 et`) y, con
/// `layout`, el estilo de las líneas que comparte con el editor. Devuelve el
/// texto y el modo.
fn run(before: &str, keys: &str, markdown: Markdown, layout: bool) -> (String, String) {
    let (mut buffer, mut selections) = support::parse(before);
    let mut vim = markdown_vim(&buffer, layout);
    vim.set_markdown(markdown);
    let toggle = Action::from_name("editor::toggle_checkbox").unwrap();
    let mut pieces = keys.split("<toggle>").peekable();
    while let Some(piece) = pieces.next() {
        support::feed(&mut vim, &mut buffer, &mut selections, piece);
        if pieces.peek().is_some() {
            vim.execute(toggle, &mut buffer, &mut selections).unwrap();
        }
    }
    let text = support::render(&buffer, heads(&selections));
    (text, support::mode_name(vim.mode()).to_owned())
}

fn markdown_vim(buffer: &Buffer, layout: bool) -> Vim {
    let mut vim = Vim::new();
    vim.set_indent(Indent {
        shiftwidth: NonZeroUsize::new(4).unwrap(),
        expandtab: true,
    });
    let mut viewport = Viewport::default();
    viewport.set_tabstop(4);
    vim.swap_viewport(&mut viewport);
    let mut layout = layout.then(|| Layout::new(buffer));
    vim.swap_layout(&mut layout);
    vim
}

fn heads(selections: &Selections) -> Vec<metalmind_buffer::ByteOffset> {
    selections.iter().map(|s| s.head).collect()
}
