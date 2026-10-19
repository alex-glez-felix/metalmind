use std::fs;
use std::path::{Path, PathBuf};

use metalmind_vault::{Vault, VaultPath};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::workspace::Message;
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
    assert_eq!(session.workspace.documents.iter().count(), 2);
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

/// Como `:e` con `hidden`: la nota con cambios queda oculta y vuelve
/// igual, con el cursor donde se dejó.
#[test]
fn con_cambios_cambia_y_la_nota_queda_oculta() {
    let (_dir, vault, mut session) = vault();
    session.keys("ix⎋");
    let opened = session
        .workspace
        .open_file(&file(&vault, "b.md"), Placement::Current)
        .unwrap();
    assert!(opened);
    assert_eq!(shown(&session), file(&vault, "b.md"));
    assert!(!matches!(
        session.workspace.message(),
        Some(Message::Error(_))
    ));
    session
        .workspace
        .open_file(&file(&vault, "a.md"), Placement::Current)
        .unwrap();
    assert_eq!(session.text(), "xa");
    assert!(session.is_modified());
    assert_eq!(session.cursor(), metalmind_buffer::ByteOffset::ZERO);
}

/// Una nota oculta con cambios también va a la papelera, sin volver.
#[test]
fn borrar_suelta_la_nota_oculta() {
    let (_dir, vault, mut session) = vault();
    session.keys("x");
    session
        .workspace
        .open_file(&file(&vault, "b.md"), Placement::Current)
        .unwrap();
    session
        .workspace
        .trash_file(&vault, &vault_path("a.md"))
        .unwrap();
    assert_eq!(shown(&session), file(&vault, "b.md"));
    assert_eq!(session.workspace.documents.iter().count(), 1);
    assert!(session.workspace.hidden.is_empty());
    assert!(session.workspace.may_close_window());
}

/// Renombrar una nota oculta la lleva a su ruta nueva con sus cambios.
#[test]
fn renombrar_lleva_la_nota_oculta() {
    let (_dir, vault, mut session) = vault();
    session.keys("x");
    session
        .workspace
        .open_file(&file(&vault, "b.md"), Placement::Current)
        .unwrap();
    session
        .workspace
        .rename_file(&vault, &vault_path("a.md"), &vault_path("z.md"))
        .unwrap();
    session
        .workspace
        .open_file(&file(&vault, "z.md"), Placement::Current)
        .unwrap();
    assert_eq!(session.text(), "");
    assert!(session.is_modified());
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
    assert_eq!(
        session.view(session.pane()).mode,
        metalmind_vim::Mode::Normal
    );
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

#[test]
fn la_ruta_se_muestra_desde_el_directorio_de_trabajo() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(shown_path(&cwd.join("sub/a.md")), Path::new("sub/a.md"));
    assert_eq!(
        shown_path(Path::new("/fuera/a.md")),
        Path::new("/fuera/a.md")
    );
    assert_eq!(shown_path(&cwd), cwd, "el directorio mismo no queda vacío");
}

/// Un documento abierto por un enlace dentro de la carpeta movida sigue a
/// la carpeta aunque el enlace apunte fuera de ella: lo que se mueve es la
/// entrada, no su destino.
#[test]
fn mover_una_carpeta_lleva_los_buffers_abiertos_por_un_enlace() {
    let (dir, vault, mut session) = vault();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("real.md");
    fs::write(&target, "real\n").unwrap();
    std::os::unix::fs::symlink(&target, file(&vault, "notas/enlace.md")).unwrap();
    session
        .workspace
        .open_file(
            &file(&vault, "notas/enlace.md"),
            Placement::Split(Axis::Row),
        )
        .unwrap();
    session.keys("x");
    assert!(session.workspace.has_unsaved_within(&file(&vault, "notas")));
    session
        .workspace
        .rename_file(&vault, &vault_path("notas"), &vault_path("movidas"))
        .unwrap();
    assert_eq!(shown(&session), file(&vault, "movidas/enlace.md"));
    assert!(session.view(session.pane()).document.is_modified());
    session.keys(":w⏎");
    assert_eq!(fs::read_to_string(&target).unwrap(), "eal\n");
    drop(dir);
}

/// Renombrar el enlace mismo lleva al buffer abierto por él.
#[test]
fn renombrar_un_enlace_lleva_su_buffer() {
    let (_dir, vault, mut session) = vault();
    std::os::unix::fs::symlink(file(&vault, "b.md"), file(&vault, "enlace.md")).unwrap();
    session
        .workspace
        .open_file(&file(&vault, "enlace.md"), Placement::Current)
        .unwrap();
    session
        .workspace
        .rename_file(&vault, &vault_path("enlace.md"), &vault_path("otro.md"))
        .unwrap();
    assert_eq!(shown(&session), file(&vault, "otro.md"));
}

fn at(line: usize, column: usize) -> LinePosition {
    LinePosition { line, column }
}

/// Lo que abre `space /`: la nota con el cursor en la coincidencia.
#[test]
fn abrir_en_una_linea_pone_el_cursor_en_la_columna() {
    let (_dir, vault, mut session) = vault();
    fs::write(file(&vault, "b.md"), "uno\ndos\nuna idea\n").unwrap();
    let opened = session
        .workspace
        .open_file_at(&file(&vault, "b.md"), Placement::Current, at(2, 4))
        .unwrap();
    assert!(opened);
    assert_eq!(shown(&session), file(&vault, "b.md"));
    assert_eq!(session.line(session.pane()), 3);
    assert_eq!(session.cursor().get(), 12);
    session.keys("x");
    assert_eq!(session.text(), "uno\ndos\nuna dea");
}

#[test]
fn abrir_en_una_linea_en_un_split_mueve_solo_el_panel_nuevo() {
    let (_dir, vault, mut session) = vault();
    fs::write(file(&vault, "notas/c.md"), "c\notra c\n").unwrap();
    let first = session.pane();
    let opened = session
        .workspace
        .open_file_at(
            &file(&vault, "notas/c.md"),
            Placement::Split(Axis::Row),
            at(1, 5),
        )
        .unwrap();
    assert!(opened);
    assert_ne!(session.pane(), first);
    assert_eq!(session.cursor().get(), 7);
    assert_eq!(session.view(first).cursor().get(), 0);
}

/// El archivo pudo cambiar desde la búsqueda: el cursor queda en el texto,
/// en frontera de carácter y nunca sobre el salto de línea.
#[test]
fn una_posicion_que_ya_no_existe_se_acerca_al_texto() {
    let (_dir, vault, mut session) = vault();
    fs::write(file(&vault, "b.md"), "año\ncorta\n").unwrap();
    let b = file(&vault, "b.md");
    session
        .workspace
        .open_file_at(&b, Placement::Current, at(0, 2))
        .unwrap();
    assert_eq!(session.cursor().get(), 1, "no parte la ñ");
    session
        .workspace
        .open_file_at(&b, Placement::Current, at(1, 40))
        .unwrap();
    assert_eq!(session.cursor().get(), 9, "la última letra de la línea");
    session
        .workspace
        .open_file_at(&b, Placement::Current, at(30, 0))
        .unwrap();
    assert_eq!(session.line(session.pane()), 2, "la última línea");
}

/// Una carpeta no se puede leer como nota: no cambia nada.
#[test]
fn si_no_se_abre_el_cursor_no_se_mueve() {
    let (_dir, vault, mut session) = vault();
    let opened = session
        .workspace
        .open_file_at(&file(&vault, "notas"), Placement::Current, at(0, 1))
        .unwrap();
    assert!(!opened);
    assert_eq!(shown(&session), file(&vault, "a.md"));
    assert_eq!(session.cursor().get(), 0);
}

/// Con `hidden`, una nota con cambios no impide abrir el resultado: queda
/// oculta con sus cambios y `ctrl-o` vuelve a ella.
#[test]
fn abrir_en_una_linea_con_cambios_deja_la_nota_oculta() {
    let (_dir, vault, mut session) = vault();
    fs::write(file(&vault, "b.md"), "uno\ndos\n").unwrap();
    session.keys("x");
    let opened = session
        .workspace
        .open_file_at(&file(&vault, "b.md"), Placement::Current, at(1, 0))
        .unwrap();
    assert!(opened);
    assert_eq!(session.line(session.pane()), 2);
    session.keys("⌃o");
    assert_eq!(shown(&session), file(&vault, "a.md"));
    assert_eq!(session.text(), "", "con sus cambios");
    assert!(session.is_modified());
}

/// Abrir un resultado de `space /` es un salto: `ctrl-o` vuelve.
#[test]
fn abrir_en_una_linea_queda_en_la_jumplist() {
    let (_dir, vault, mut session) = vault();
    fs::write(file(&vault, "b.md"), "uno\ndos\ntres\n").unwrap();
    let b = file(&vault, "b.md");
    session
        .workspace
        .open_file_at(&b, Placement::Current, at(2, 0))
        .unwrap();
    session
        .workspace
        .open_file_at(&b, Placement::Current, at(1, 1))
        .unwrap();
    assert_eq!(session.cursor().get(), 5);
    session.keys("⌃o");
    assert_eq!(session.line(session.pane()), 3, "dentro de la misma nota");
    session.keys("⌃o");
    assert_eq!(
        shown(&session),
        file(&vault, "a.md"),
        "y a la nota de antes"
    );
}

/// Saltar dentro de la nota que se ve termina lo que estuviera a medias, como
/// `:e` y el split (la paleta puede abrir `space /` desde Insert o Visual).
#[test]
fn abrir_en_la_nota_que_se_ve_termina_insert_y_visual() {
    let (_dir, vault, mut session) = vault();
    fs::write(file(&vault, "b.md"), "uno\ndos\n").unwrap();
    let b = file(&vault, "b.md");
    session
        .workspace
        .open_file_at(&b, Placement::Current, at(0, 0))
        .unwrap();
    for keys in ["i", "v"] {
        session.keys(keys);
        assert_ne!(session.mode(), metalmind_vim::Mode::Normal);
        session
            .workspace
            .open_file_at(&b, Placement::Current, at(1, 1))
            .unwrap();
        assert_eq!(session.mode(), metalmind_vim::Mode::Normal, "tras {keys}");
        assert_eq!(session.cursor().get(), 5);
    }
}
