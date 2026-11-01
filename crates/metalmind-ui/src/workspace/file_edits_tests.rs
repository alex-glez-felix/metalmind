use std::fs;
use std::path::PathBuf;

use metalmind_buffer::ByteOffset;
use metalmind_vault::{FileEdits, Mismatch, TextEdit, VaultPath};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::workspace::Axis;
use crate::workspace::Placement;
use crate::workspace::test_session::Session;

const NOTE: &str = "#rust uno\nlínea #rust\n";

/// `a.md` con [`NOTE`] y `b.md`, y una sesión en `a.md` con un cambio sin
/// guardar al final de la segunda línea y el cursor ahí.
fn session() -> (TempDir, Session) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), NOTE).unwrap();
    fs::write(dir.path().join("b.md"), "b\n").unwrap();
    let mut session = Session::open(&dir.path().join("a.md"));
    session.keys("jA!⎋");
    (dir, session)
}

fn file(dir: &TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

/// `#rust` → `#lenguaje` en las dos líneas de `a.md`, sobre el texto del
/// buffer (con el `!`).
fn rename_rust() -> FileEdits {
    let edit = |range: std::ops::Range<usize>, line| TextEdit {
        range,
        old: "#rust".to_owned(),
        new: "#lenguaje".to_owned(),
        line,
    };
    FileEdits {
        path: VaultPath::new("a.md").unwrap(),
        edits: vec![edit(0..5, 0), edit(17..22, 1)],
        base_len: None,
    }
}

/// Entra como un solo paso de undo, el cursor sigue a su texto y el disco
/// no se toca: el documento queda con cambios sin guardar.
#[test]
fn entra_en_el_undo_sin_tocar_el_disco() {
    let (dir, mut session) = session();
    let undo = session
        .workspace
        .edit_file(&file(&dir, "a.md"), &rename_rust())
        .unwrap();
    assert_eq!(session.text(), "#lenguaje uno\nlínea #lenguaje!");
    assert_eq!(session.cursor(), ByteOffset::new(30));
    assert!(session.is_modified());
    assert_eq!(fs::read_to_string(file(&dir, "a.md")).unwrap(), NOTE);

    let (back, _) = undo.apply(&session.text()).unwrap();
    assert_eq!(back, "#rust uno\nlínea #rust!");
    session.keys("u");
    assert_eq!(session.text(), "#rust uno\nlínea #rust!");
}

/// Con el documento en dos paneles, el cursor de cada uno sigue al texto.
#[test]
fn sigue_el_cursor_de_cada_panel() {
    let (dir, mut session) = session();
    let first = session.pane();
    session
        .workspace
        .open_file(&file(&dir, "a.md"), Placement::Split(Axis::Column))
        .unwrap();
    session.keys("gg$");
    let second = session.pane();
    session
        .workspace
        .edit_file(&file(&dir, "a.md"), &rename_rust())
        .unwrap();
    let cursor = |pane| session.view(pane).selection.head;
    assert_eq!(cursor(first), ByteOffset::new(30));
    assert_eq!(cursor(second), ByteOffset::new(12));
}

/// Un documento oculto también: al volver tiene el texto nuevo, el cursor
/// sigue en su sitio y `u` lo deshace.
#[test]
fn edita_un_documento_oculto() {
    let (dir, mut session) = session();
    session
        .workspace
        .open_file(&file(&dir, "b.md"), Placement::Current)
        .unwrap();
    session
        .workspace
        .edit_file(&file(&dir, "a.md"), &rename_rust())
        .unwrap();
    session
        .workspace
        .open_file(&file(&dir, "a.md"), Placement::Current)
        .unwrap();
    assert_eq!(session.text(), "#lenguaje uno\nlínea #lenguaje!");
    assert_eq!(session.cursor(), ByteOffset::new(30));
    assert!(session.is_modified());
    session.keys("u");
    assert_eq!(session.text(), "#rust uno\nlínea #rust!");
}

/// Si el texto ya no dice lo que esperan, no cambia nada.
#[test]
fn texto_que_cambio_no_aplica_nada() {
    let (dir, mut session) = session();
    session.keys("gg0x");
    let path = file(&dir, "a.md");
    let error = session
        .workspace
        .edit_file(&path, &rename_rust())
        .unwrap_err();
    assert!(
        matches!(
            &error,
            FileEditError::Stale(stale) if matches!(stale.mismatch, Mismatch::Text { .. })
        ),
        "{error}"
    );
    assert!(
        session
            .workspace
            .check_file_edits(&path, &rename_rust())
            .is_err()
    );
    assert_eq!(session.text(), "rust uno\nlínea #rust!");
}

#[test]
fn sin_documento_abierto_no_aplica() {
    let (dir, mut session) = session();
    let error = session
        .workspace
        .edit_file(&file(&dir, "b.md"), &rename_rust())
        .unwrap_err();
    assert!(matches!(error, FileEditError::NotOpen(_)), "{error}");
}

/// A mitad de un Insert no se mezcla con lo tecleado.
#[test]
fn con_un_insert_abierto_no_aplica() {
    let (dir, mut session) = session();
    session.keys("A?");
    let error = session
        .workspace
        .edit_file(&file(&dir, "a.md"), &rename_rust())
        .unwrap_err();
    assert!(matches!(error, FileEditError::Typing(_)), "{error}");
    session.keys("⎋");
    assert_eq!(session.text(), "#rust uno\nlínea #rust!?");
}

fn one_edit(range: std::ops::Range<usize>, old: &str, new: &str) -> FileEdits {
    FileEdits {
        path: VaultPath::new("a.md").unwrap(),
        edits: vec![TextEdit {
            range,
            old: old.to_owned(),
            new: new.to_owned(),
            line: 0,
        }],
        base_len: None,
    }
}

/// Las ediciones se planean sobre el archivo como se guardaría, con su
/// `\n` final, que el buffer no tiene: una que lo reemplaza y lo vuelve a
/// poner, o que agrega después de él, queda igual al guardar.
#[test]
fn el_salto_final_del_archivo_se_queda_fuera_del_buffer() {
    let (dir, mut session) = session();
    let path = file(&dir, "a.md");
    let text = session.workspace.file_text(&path).unwrap();
    assert_eq!(text, "#rust uno\nlínea #rust!\n");
    let end = text.len();
    session
        .workspace
        .edit_file(&path, &one_edit(end - 2..end, "!\n", "?\n"))
        .unwrap();
    session
        .workspace
        .edit_file(&path, &one_edit(end..end, "", "---\n"))
        .unwrap();
    assert_eq!(
        session.workspace.file_text(&path).unwrap(),
        "#rust uno\nlínea #rust?\n---\n"
    );
    let error = session
        .workspace
        .edit_file(&path, &one_edit(end + 2..end + 4, "-\n", "-"))
        .unwrap_err();
    assert!(matches!(error, FileEditError::FinalNewline(_)), "{error}");
}

/// En un archivo vacío no hay `\n` final: el de lo agregado es el que
/// pone el guardado.
#[test]
fn en_un_archivo_vacio_lo_agregado_trae_su_salto() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.md");
    fs::write(&path, "").unwrap();
    let mut session = Session::open(&path);
    session.keys("ix⎋x");
    assert_eq!(session.workspace.file_text(&path).unwrap(), "");
    session
        .workspace
        .edit_file(&path, &one_edit(0..0, "", "---\ntags: [a]\n---\n"))
        .unwrap();
    assert_eq!(
        session.workspace.file_text(&path).unwrap(),
        "---\ntags: [a]\n---\n"
    );
}
