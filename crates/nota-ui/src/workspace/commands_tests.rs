use std::fs;
use std::path::{Path, PathBuf};

use nota_buffer::ByteOffset;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::workspace::test_session::Session;

/// Una carpeta con `a.md` y `b.md`, y una sesión abierta en `a.md`.
fn two_files() -> (TempDir, Session) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "a\n").unwrap();
    fs::write(dir.path().join("b.md"), "b1\nb2\n").unwrap();
    let session = Session::open(&dir.path().join("a.md"));
    (dir, session)
}

fn path(dir: &TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

fn edit(session: &mut Session, command: &str, path: &Path) {
    session.keys(&format!(":{command} {}⏎", path.display()));
}

fn shown(session: &Session) -> PathBuf {
    session.view(session.pane()).document.path().to_owned()
}

fn error(session: &Session) -> Option<&str> {
    match session.workspace.message() {
        Some(Message::Error(text)) => Some(text),
        _ => None,
    }
}

#[test]
fn e_abre_otro_archivo_en_el_panel() {
    let (dir, mut session) = two_files();
    edit(&mut session, "e", &path(&dir, "b.md"));
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(session.text(), "b1\nb2");
    assert_eq!(session.cursor(), ByteOffset::ZERO);
    assert_eq!(session.workspace.documents.iter().count(), 1);
    let opened = format!("\"{}\" 2L, 6B", path(&dir, "b.md").display());
    assert_eq!(session.workspace.message(), Some(&Message::Info(opened)));
}

#[test]
fn e_de_un_archivo_sin_salto_final_avisa_noeol() {
    let (dir, mut session) = two_files();
    fs::write(path(&dir, "c.md"), "c1\nc2").unwrap();
    edit(&mut session, "e", &path(&dir, "c.md"));
    let opened = format!("\"{}\" [noeol] 2L, 5B", path(&dir, "c.md").display());
    assert_eq!(session.workspace.message(), Some(&Message::Info(opened)));
}

/// Vaciado en otro panel, ya no es lo que hay en disco: el aviso cuenta el buffer.
#[test]
fn e_de_un_archivo_sin_salto_final_vaciado_en_otro_panel() {
    let (dir, mut session) = two_files();
    fs::write(path(&dir, "c.md"), "c").unwrap();
    session.keys("⌃wv");
    edit(&mut session, "e", &path(&dir, "c.md"));
    session.keys("dd⌃wh");
    assert_eq!(shown(&session), path(&dir, "a.md"));
    edit(&mut session, "e", &path(&dir, "c.md"));
    let opened = format!("\"{}\" 0L, 0B", path(&dir, "c.md").display());
    assert_eq!(session.workspace.message(), Some(&Message::Info(opened)));
}

#[test]
fn e_de_un_archivo_que_no_existe_lo_empieza_vacio() {
    let (dir, mut session) = two_files();
    edit(&mut session, "e", &path(&dir, "nueva.md"));
    assert_eq!(session.text(), "");
    assert!(!session.exists_on_disk());
    session.keys("ihola⎋:w⏎");
    assert_eq!(
        fs::read_to_string(path(&dir, "nueva.md")).unwrap(),
        "hola\n"
    );
}

#[test]
fn e_se_niega_si_se_perderian_cambios() {
    let (dir, mut session) = two_files();
    session.keys("x");
    edit(&mut session, "e", &path(&dir, "b.md"));
    assert_eq!(shown(&session), path(&dir, "a.md"));
    assert_eq!(session.text(), "");
    assert_eq!(
        error(&session),
        Some("E37: hay cambios sin guardar (usa :e! para descartarlos)")
    );
}

#[test]
fn e_con_bang_descarta_los_cambios() {
    let (dir, mut session) = two_files();
    session.keys("x");
    edit(&mut session, "e!", &path(&dir, "b.md"));
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(session.workspace.documents.iter().count(), 1);
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.text(), "a");
}

#[test]
fn e_cambia_si_el_buffer_sigue_en_otro_panel() {
    let (dir, mut session) = two_files();
    session.keys("x⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(session.workspace.documents.iter().count(), 2);
    session.keys("⌃wh");
    assert_eq!(session.text(), "");
    assert!(session.is_modified());
}

#[test]
fn e_de_un_archivo_abierto_comparte_el_documento() {
    let (dir, mut session) = two_files();
    session.keys("⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("j");
    let right = session.pane();
    session.keys("⌃wh");
    edit(&mut session, "e", &dir.path().join(".").join("b.md"));
    assert_eq!(session.workspace.documents.iter().count(), 1);
    assert_eq!(session.cursor(), ByteOffset::new(3));
    session.keys("x");
    assert_eq!(session.view(right).buffer().to_string(), "b1\n2");
}

#[test]
fn e_sin_ruta_relee_el_archivo() {
    let (dir, _) = two_files();
    fs::write(path(&dir, "a.md"), "a\nz\n").unwrap();
    let mut session = Session::open(&path(&dir, "a.md"));
    session.keys("j");
    fs::write(path(&dir, "a.md"), "a\nfuera\n").unwrap();
    session.keys(":e⏎");
    assert_eq!(session.text(), "a\nfuera");
    assert_eq!(session.cursor(), ByteOffset::new(2));
    session.keys("x");
    session.keys(":e⏎");
    assert_eq!(session.text(), "a\nuera");
    assert!(error(&session).is_some());
    session.keys(":e!⏎");
    assert_eq!(session.text(), "a\nfuera");
    assert!(!session.is_modified());
}

#[test]
fn e_con_la_ruta_del_mismo_archivo_relee() {
    let (dir, mut session) = two_files();
    session.keys("x");
    edit(&mut session, "e!", &path(&dir, "a.md"));
    assert_eq!(session.text(), "a");
    assert!(!session.is_modified());
}

#[test]
fn e_de_algo_ilegible_lo_avisa_y_no_cambia_nada() {
    let (dir, mut session) = two_files();
    fs::write(path(&dir, "binario.md"), [0xff, 0xfe]).unwrap();
    edit(&mut session, "e", &path(&dir, "binario.md"));
    assert_eq!(shown(&session), path(&dir, "a.md"));
    assert!(error(&session).is_some());
}

#[test]
fn q_cierra_el_panel_y_el_ultimo_la_ventana() {
    let mut session = Session::new("abc\n");
    let left = session.pane();
    session.keys("⌃wv");
    session.keys(":q⏎");
    assert_eq!(session.effect, Effect::Stay);
    assert_eq!(session.workspace.tree().panes(), [left]);
    assert_eq!(session.pane(), left);
    session.keys("⌃wq");
    assert_eq!(session.effect, Effect::CloseWindow);
}

#[test]
fn q_cierra_un_panel_con_cambios_si_otro_muestra_el_buffer() {
    let mut session = Session::new("abc\n");
    session.keys("x⌃wv:q⏎");
    assert_eq!(session.workspace.tree().len(), 1);
    assert!(session.is_modified());
    session.keys(":q⏎");
    assert_eq!(session.effect, Effect::Stay);
    assert_eq!(
        error(&session),
        Some("E37: hay cambios sin guardar (usa :q! para cerrar igual)")
    );
    session.keys(":q!⏎");
    assert_eq!(session.effect, Effect::CloseWindow);
}

#[test]
fn q_se_niega_si_el_panel_es_el_unico_con_esos_cambios() {
    let (dir, mut session) = two_files();
    session.keys("⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("x:q⏎");
    assert_eq!(session.workspace.tree().len(), 2);
    assert!(error(&session).is_some());
    session.keys(":q!⏎");
    assert_eq!(session.workspace.tree().len(), 1);
    assert_eq!(session.workspace.documents.iter().count(), 1);
    assert_eq!(shown(&session), path(&dir, "a.md"));
}

#[test]
fn wq_guarda_y_cierra_el_panel() {
    let (dir, mut session) = two_files();
    session.keys("⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("x:wq⏎");
    assert_eq!(fs::read_to_string(path(&dir, "b.md")).unwrap(), "1\nb2\n");
    assert_eq!(session.workspace.tree().len(), 1);
    session.keys(":wq⏎");
    assert_eq!(session.effect, Effect::CloseWindow);
}

#[test]
fn cerrar_la_ventana_mira_todos_los_buffers() {
    let (dir, mut session) = two_files();
    session.keys("x⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    assert!(!session.workspace.may_close_window());
    let unsaved = format!(
        "E162: hay cambios sin guardar en \"{}\"",
        path(&dir, "a.md").display()
    );
    assert_eq!(error(&session), Some(unsaved.as_str()));
    session.keys("⌃wh:w⏎");
    assert!(session.workspace.may_close_window());
}

#[test]
fn dividir_copia_el_cursor_y_moverse_cambia_el_foco() {
    let mut session = Session::new("uno dos\n");
    session.keys("w⌃ws");
    let bottom = session.pane();
    assert_eq!(session.cursor(), ByteOffset::new(4));
    session.keys("⌃wk");
    assert_ne!(session.pane(), bottom);
    session.keys("⌃wj");
    assert_eq!(session.pane(), bottom);
    session.keys("⌃ww");
    assert_ne!(session.pane(), bottom);
    session.keys("⌃wl");
    assert_ne!(session.pane(), bottom);
}

#[test]
fn only_cierra_los_demas_salvo_los_que_perderian_cambios() {
    let (dir, mut session) = two_files();
    session.keys("⌃wv⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("x⌃wv");
    let keep = session.pane();
    edit(&mut session, "e", &path(&dir, "nueva.md"));
    session.keys("⌃wo");
    assert_eq!(session.workspace.tree().len(), 2);
    assert_eq!(session.pane(), keep);
    assert_eq!(
        error(&session),
        Some("E445: otros paneles tienen cambios sin guardar")
    );
    assert_eq!(session.workspace.documents.iter().count(), 2);
}

#[test]
fn only_con_un_solo_panel_no_hace_nada() {
    let mut session = Session::new("abc\n");
    session.keys(":on⏎");
    assert_eq!(session.workspace.tree().len(), 1);
    assert_eq!(error(&session), None);
}

#[cfg(unix)]
#[test]
fn e_no_abre_otro_buffer_del_archivo_al_que_ya_escribe_un_enlace() {
    let dir = TempDir::new().unwrap();
    let (a, b, link) = (
        dir.path().join("a.md"),
        dir.path().join("b.md"),
        dir.path().join("enlace.md"),
    );
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    std::os::unix::fs::symlink(&a, &link).unwrap();
    let mut session = Session::open(&link);
    session.keys("x⌃wv");
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&b, &link).unwrap();
    edit(&mut session, "e", &b);
    assert_eq!(
        error(&session),
        Some("E37: hay cambios sin guardar (usa :e! para descartarlos)")
    );
    assert_eq!(session.workspace.documents.iter().count(), 1);
}

#[test]
fn saltar_tras_e_no_usa_las_marks_del_otro_buffer() {
    let (dir, mut session) = two_files();
    session.keys("G");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("G");
    assert_eq!(error(&session), None);
    assert_eq!(session.cursor(), ByteOffset::new(3));
    session.keys("x:e!⏎gg");
    assert_eq!(error(&session), None);
    assert_eq!(session.cursor(), ByteOffset::ZERO);
}

#[test]
fn cada_buffer_tiene_sus_marks() {
    let (dir, _) = two_files();
    fs::write(path(&dir, "a.md"), "a\nz\n").unwrap();
    let mut session = Session::open(&path(&dir, "a.md"));
    session.keys("jma⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("jmbgg'a");
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(session.cursor(), ByteOffset::ZERO);
    session.keys("'b⌃whgg'a");
    assert_eq!(shown(&session), path(&dir, "a.md"));
    assert_eq!(session.cursor(), ByteOffset::new(2));
}

fn numbers(session: &Session, pane: PaneId) -> LineNumbers {
    session.view(pane).numbers
}

#[test]
fn set_cambia_los_numeros_solo_del_panel_enfocado() {
    let mut session = Session::new("uno\n");
    let left = session.pane();
    assert_eq!(numbers(&session, left), LineNumbers::HYBRID);
    session.keys("⌃wv:set nornu⏎");
    let right = session.pane();
    let absolute = LineNumbers {
        absolute: true,
        relative: false,
    };
    assert_eq!(numbers(&session, right), absolute);
    assert_eq!(numbers(&session, left), LineNumbers::HYBRID);
    session.keys("⌃ws");
    assert_eq!(
        numbers(&session, session.pane()),
        absolute,
        "el split hereda"
    );
    session.keys(":se nu!⏎");
    assert!(!numbers(&session, session.pane()).shown());
}

#[test]
fn e_no_cambia_los_numeros_del_panel() {
    let (dir, mut session) = two_files();
    session.keys(":set nonu⏎");
    edit(&mut session, "e", &path(&dir, "b.md"));
    let expected = LineNumbers {
        absolute: false,
        relative: true,
    };
    assert_eq!(numbers(&session, session.pane()), expected);
}

#[test]
fn set_con_una_opcion_desconocida_avisa() {
    let mut session = Session::new("uno\n");
    session.keys(":set wrap⏎");
    assert_eq!(
        session.workspace.message(),
        Some(&Message::Error("E518: opción desconocida: wrap".to_owned()))
    );
    assert_eq!(numbers(&session, session.pane()), LineNumbers::HYBRID);
}
