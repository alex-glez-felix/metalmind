use std::fs;
use std::path::{Path, PathBuf};

use nota_vault::{Vault, VaultPath};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::workspace::test_session::Session;

/// Un vault con `a.md`, `b.md` y `notas/c.md`, y una sesión abierta en `a.md`.
fn vault() -> (TempDir, Vault, Session) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "a\n").unwrap();
    fs::write(dir.path().join("b.md"), "b\n").unwrap();
    fs::create_dir(dir.path().join("notas")).unwrap();
    fs::write(dir.path().join("notas/c.md"), "c\n").unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let session = Session::open(&vault.root().join("a.md"));
    (dir, vault, session)
}

fn file(vault: &Vault, name: &str) -> PathBuf {
    vault.root().join(name)
}

fn vault_path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn shown(session: &Session) -> PathBuf {
    let view = session.view(session.pane());
    view.document.path().unwrap().to_owned()
}

#[test]
fn abrir_en_el_panel_reemplaza_lo_que_muestra() {
    let (_dir, vault, mut session) = vault();
    let opened = session
        .workspace
        .open_file(&file(&vault, "b.md"), Placement::Current)
        .unwrap();
    assert!(opened);
    assert_eq!(shown(&session), file(&vault, "b.md"));
    assert_eq!(session.workspace.tree().len(), 1);
    assert_eq!(session.workspace.documents.iter().count(), 1);
}

/// `:e` del mismo archivo lo relee; abrirlo desde el árbol no lo toca.
#[test]
fn abrir_el_archivo_que_ya_muestra_no_lo_relee() {
    let (_dir, vault, mut session) = vault();
    session.keys("x");
    let before = session.workspace.message().cloned();
    let opened = session
        .workspace
        .open_file(&file(&vault, "a.md"), Placement::Current)
        .unwrap();
    assert!(opened);
    assert_eq!(session.text(), "");
    assert_eq!(session.workspace.message(), before.as_ref());
}

#[test]
fn con_cambios_se_niega_como_e() {
    let (_dir, vault, mut session) = vault();
    session.keys("x");
    let opened = session
        .workspace
        .open_file(&file(&vault, "b.md"), Placement::Current)
        .unwrap();
    assert!(!opened);
    assert_eq!(shown(&session), file(&vault, "a.md"));
    assert!(matches!(
        session.workspace.message(),
        Some(Message::Error(text)) if text.starts_with("E37")
    ));
}

#[test]
fn abrir_en_split_deja_el_panel_anterior_como_estaba() {
    let (_dir, vault, mut session) = vault();
    session.keys("x");
    let before = session.pane();
    let opened = session
        .workspace
        .open_file(&file(&vault, "notas/c.md"), Placement::Split(Axis::Row))
        .unwrap();
    assert!(opened);
    assert_ne!(session.pane(), before);
    assert_eq!(shown(&session), file(&vault, "notas/c.md"));
    assert_eq!(session.text(), "c");
    let other = session.view(before);
    assert_eq!(other.document.path(), Some(file(&vault, "a.md").as_path()));
    assert!(other.document.is_modified());
}

#[test]
fn un_archivo_que_no_se_lee_no_abre_split() {
    let (_dir, vault, mut session) = vault();
    fs::write(file(&vault, "imagen.png"), [0xff, 0xfe, 0x00]).unwrap();
    let opened = session
        .workspace
        .open_file(&file(&vault, "imagen.png"), Placement::Split(Axis::Column))
        .unwrap();
    assert!(!opened);
    assert_eq!(session.workspace.tree().len(), 1);
    assert!(matches!(
        session.workspace.message(),
        Some(Message::Error(_))
    ));
}

#[test]
fn renombrar_lleva_el_buffer_abierto_con_sus_cambios() {
    let (_dir, vault, mut session) = vault();
    session.keys("x");
    session
        .workspace
        .rename_file(&vault, &vault_path("a.md"), &vault_path("z.md"))
        .unwrap();
    assert_eq!(shown(&session), file(&vault, "z.md"));
    assert!(session.view(session.pane()).document.is_modified());
    session.keys(":w⏎");
    assert_eq!(fs::read_to_string(file(&vault, "z.md")).unwrap(), "");
    assert!(!file(&vault, "a.md").exists());
}

#[test]
fn mover_una_carpeta_lleva_los_buffers_de_adentro() {
    let (_dir, vault, mut session) = vault();
    session
        .workspace
        .open_file(&file(&vault, "notas/c.md"), Placement::Split(Axis::Row))
        .unwrap();
    session
        .workspace
        .rename_file(&vault, &vault_path("notas"), &vault_path("archivo/viejas"))
        .unwrap();
    assert_eq!(shown(&session), file(&vault, "archivo/viejas/c.md"));
    assert!(
        session
            .workspace
            .documents
            .find(&file(&vault, "a.md"))
            .is_some()
    );
}

#[test]
fn si_renombrar_falla_los_buffers_no_cambian() {
    let (_dir, vault, mut session) = vault();
    let error = session
        .workspace
        .rename_file(&vault, &vault_path("a.md"), &vault_path("b.md"));
    assert!(error.is_err());
    assert_eq!(shown(&session), file(&vault, "a.md"));
}

#[test]
fn sabe_si_hay_cambios_sin_guardar_dentro_de_una_ruta() {
    let (_dir, vault, mut session) = vault();
    session
        .workspace
        .open_file(&file(&vault, "notas/c.md"), Placement::Split(Axis::Row))
        .unwrap();
    assert!(!session.workspace.has_unsaved_within(&file(&vault, "notas")));
    session.keys("x");
    assert!(session.workspace.has_unsaved_within(&file(&vault, "notas")));
    assert!(session.workspace.has_unsaved_within(vault.root()));
    assert!(!session.workspace.has_unsaved_within(&file(&vault, "a.md")));
    assert!(
        !session
            .workspace
            .has_unsaved_within(Path::new("/no/existe"))
    );
}

#[test]
fn borrar_cierra_los_paneles_de_lo_borrado() {
    let (_dir, vault, mut session) = vault();
    session
        .workspace
        .open_file(&file(&vault, "notas/c.md"), Placement::Split(Axis::Row))
        .unwrap();
    session.keys("ix");
    session
        .workspace
        .trash_file(&vault, &vault_path("notas"))
        .unwrap();
    assert_eq!(session.workspace.tree().len(), 1);
    assert_eq!(shown(&session), file(&vault, "a.md"));
    assert_eq!(session.view(session.pane()).mode, nota_vim::Mode::Normal);
    assert!(
        session
            .workspace
            .documents
            .find(&file(&vault, "notas/c.md"))
            .is_none()
    );
    assert!(!file(&vault, "notas").exists());
    assert!(vault.root().join(".trash/notas/c.md").exists());
}

#[test]
fn borrar_lo_del_ultimo_panel_lo_deja_sin_nombre() {
    let (_dir, vault, mut session) = vault();
    session.keys("x");
    session
        .workspace
        .trash_file(&vault, &vault_path("a.md"))
        .unwrap();
    let view = session.view(session.pane());
    assert_eq!(view.document.path(), None);
    assert!(!view.document.is_modified());
    assert_eq!(session.workspace.documents.iter().count(), 1);
    session.keys(":w⏎");
    assert!(!file(&vault, "a.md").exists(), "no vuelve a crearlo");
}

#[test]
fn si_borrar_falla_los_buffers_no_cambian() {
    let (_dir, vault, mut session) = vault();
    let error = session
        .workspace
        .trash_file(&vault, &vault_path("no-existe.md"));
    assert!(error.is_err());
    assert_eq!(shown(&session), file(&vault, "a.md"));
}
