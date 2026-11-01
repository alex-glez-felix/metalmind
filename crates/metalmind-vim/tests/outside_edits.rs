#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Ediciones que llegan de fuera del motor ([`Vim::apply_edits`]), como
//! renombrar un tag en un buffer sin guardar. No van contra el oráculo:
//! nvim no tiene una edición que no salga de una tecla.

#[allow(
    dead_code,
    reason = "cada binario de test usa solo una parte del soporte compartido"
)]
mod support;

use metalmind_buffer::{ByteOffset, Edit};
use pretty_assertions::assert_eq;
use support::Editor;

fn replace(start: usize, end: usize, text: &str) -> Edit {
    Edit {
        range: ByteOffset::new(start)..ByteOffset::new(end),
        text: text.to_owned(),
    }
}

/// `#rust` → `#lang` en dos sitios: el cursor sigue a su texto y `u` las
/// deshace juntas, con el cursor donde estaba.
#[test]
fn son_un_solo_paso_de_undo_y_el_cursor_sigue_al_texto() {
    let mut editor = Editor::new("#rust y\nmás |#rust\n");
    let edits = vec![replace(0, 5, "#lenguaje"), replace(13, 18, "#lenguaje")];
    editor.run(|vim, t| vim.apply_edits(edits, t)).unwrap();
    assert_eq!(editor.cursors(), "#lenguaje y\nmás |#lenguaje\n");
    editor.keys("u");
    assert_eq!(editor.cursors(), "#rust y\nmás |#rust\n");
    editor.keys("<C-r>");
    assert_eq!(editor.text(), "#lenguaje y\nmás #lenguaje\n");
}

/// No es un cambio para `.`: repite lo último que se tecleó.
#[test]
fn punto_repite_lo_tecleado_no_lo_aplicado() {
    let mut editor = Editor::new("|abc #a\n");
    editor.keys("x");
    editor
        .run(|vim, t| vim.apply_edits(vec![replace(3, 5, "#b")], t))
        .unwrap();
    editor.keys(".");
    assert_eq!(editor.cursors(), "|c #b\n");
}

/// Lo pendiente se cancela, como al pegar: la `d` no se come lo siguiente.
#[test]
fn cancela_el_operador_pendiente() {
    let mut editor = Editor::new("|ab #a\n");
    editor.keys("d");
    editor
        .run(|vim, t| vim.apply_edits(vec![replace(3, 5, "#b")], t))
        .unwrap();
    editor.keys("l");
    assert_eq!(editor.cursors(), "a|b #b\n");
}

/// Las marks `A`–`Z` siguen al texto, como tras cualquier edición.
#[test]
fn mueve_las_marks_de_archivo() {
    let mut editor = Editor::new("#a\n|b\n");
    editor.keys("mAgg");
    editor
        .run(|vim, t| vim.apply_edits(vec![replace(0, 2, "#a\nnueva")], t))
        .unwrap();
    editor.keys("'A");
    assert_eq!(editor.cursors(), "#a\nnueva\n|b\n");
}

/// Con un Insert abierto no se aplica nada: el Insert ya tiene su
/// transacción, que se rompería.
#[test]
fn con_un_insert_abierto_no_aplica_nada() {
    let mut editor = Editor::new("|#a\n");
    editor.keys("ix");
    let result = editor.run(|vim, t| vim.apply_edits(vec![replace(1, 3, "#b")], t));
    assert!(result.is_err());
    editor.keys("<Esc>");
    assert_eq!(editor.text(), "x#a\n");
}

/// Una edición fuera del buffer no aplica ninguna y no deja nada a medias.
#[test]
fn una_edicion_invalida_no_aplica_ninguna() {
    let mut editor = Editor::new("|#a\n");
    let edits = vec![replace(0, 2, "#b"), replace(40, 41, "x")];
    assert!(editor.run(|vim, t| vim.apply_edits(edits, t)).is_err());
    assert_eq!(editor.text(), "#a\n");
    editor.keys("x");
    editor.keys("u");
    assert_eq!(editor.text(), "#a\n");
}
