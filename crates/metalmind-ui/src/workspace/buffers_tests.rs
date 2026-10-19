use std::fs;
use std::path::{Path, PathBuf};

use metalmind_buffer::ByteOffset;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::workspace::test_session::Session;
use crate::workspace::{Effect, panes_oracle};

/// Una carpeta con `a.md`, `b.md` y `c.md`, y una sesión abierta en `a.md`.
fn three_files() -> (TempDir, Session) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "a1\na2\n").unwrap();
    fs::write(dir.path().join("b.md"), "b\n").unwrap();
    fs::write(dir.path().join("c.md"), "c\n").unwrap();
    let session = Session::open(&dir.path().join("a.md"));
    (dir, session)
}

fn path(dir: &TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

fn show(session: &mut Session, path: &Path) {
    let pane = session.pane();
    session.workspace.show_file(pane, path).unwrap();
}

fn shown(session: &Session) -> PathBuf {
    let view = session.view(session.pane());
    view.document.path().unwrap().to_owned()
}

fn error(session: &Session) -> Option<&str> {
    match session.workspace.message() {
        Some(Message::Error(text)) => Some(text),
        _ => None,
    }
}

fn unsaved(dir: &TempDir, name: &str) -> String {
    format!(
        "E162: No write since last change for buffer \"{}\"",
        path(dir, name).display()
    )
}

#[test]
fn volver_a_una_nota_con_cambios_la_trae_con_su_texto_y_su_cursor() {
    let (dir, mut session) = three_files();
    session.keys("jix⎋");
    show(&mut session, &path(&dir, "b.md"));
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(error(&session), None);
    session.keys("iy⎋");
    show(&mut session, &path(&dir, "a.md"));
    assert_eq!(session.text(), "a1\nxa2");
    assert!(session.is_modified());
    assert_eq!(session.cursor(), ByteOffset::new(3));
    show(&mut session, &path(&dir, "b.md"));
    assert_eq!(session.text(), "yb");
    assert_eq!(fs::read_to_string(path(&dir, "a.md")).unwrap(), "a1\na2\n");
}

/// Sirve para cualquier panel, no solo el enfocado, y dice qué documento
/// quedó: lo que necesita quien salta a otra nota, como la jumplist.
#[test]
fn muestra_en_el_panel_que_le_digan_y_devuelve_el_documento() {
    let (dir, mut session) = three_files();
    let left = session.pane();
    session.keys("⌃wv");
    let right = session.pane();
    let shown = session
        .workspace
        .show_file(left, &path(&dir, "b.md"))
        .unwrap();
    assert_eq!(session.pane(), right, "el foco no se mueve");
    let left_view = session.view(left);
    assert_eq!(
        left_view.document.path(),
        Some(path(&dir, "b.md").as_path())
    );
    assert!(shown.is_some());
    let again = session
        .workspace
        .show_file(left, &path(&dir, "b.md"))
        .unwrap();
    assert_eq!(again, shown);
    fs::write(path(&dir, "binario.md"), [0xff, 0xfe]).unwrap();
    let unreadable = session.workspace.show_file(left, &path(&dir, "binario.md"));
    assert_eq!(unreadable.unwrap(), None);
}

#[test]
fn mostrar_la_nota_del_panel_no_hace_nada() {
    let (dir, mut session) = three_files();
    session.keys("x");
    let before = session.workspace.message().cloned();
    show(&mut session, &path(&dir, "a.md"));
    assert_eq!(session.text(), "1\na2");
    assert_eq!(session.workspace.message(), before.as_ref());
}

/// Lo que estuviera a medias termina como con `ctrl-c`, también al
/// cambiar de nota desde el panel lateral o el finder.
#[test]
fn mostrar_otra_nota_termina_el_insert() {
    let (dir, mut session) = three_files();
    session.keys("ix");
    show(&mut session, &path(&dir, "b.md"));
    assert_eq!(session.mode(), metalmind_vim::Mode::Normal);
    show(&mut session, &path(&dir, "a.md"));
    assert_eq!(session.text(), "xa1\na2");
}

#[test]
fn una_nota_ilegible_no_cambia_el_panel() {
    let (dir, mut session) = three_files();
    fs::write(path(&dir, "binario.md"), [0xff, 0xfe]).unwrap();
    show(&mut session, &path(&dir, "binario.md"));
    assert_eq!(shown(&session), path(&dir, "a.md"));
    assert!(error(&session).is_some());
    assert_eq!(session.workspace.documents.iter().count(), 1);
}

#[test]
fn q_con_una_nota_oculta_con_cambios_la_muestra_y_no_cierra() {
    let (dir, mut session) = three_files();
    session.keys("x");
    show(&mut session, &path(&dir, "b.md"));
    session.keys(":q⏎");
    assert_eq!(session.effect, Effect::Stay);
    assert_eq!(error(&session), Some(unsaved(&dir, "a.md").as_str()));
    assert_eq!(shown(&session), path(&dir, "a.md"));
    session.keys(":q!⏎");
    assert_eq!(session.effect, Effect::CloseWindow);
}

#[test]
fn cerrar_la_ventana_con_una_nota_oculta_con_cambios_la_muestra() {
    let (dir, mut session) = three_files();
    session.keys("x");
    show(&mut session, &path(&dir, "b.md"));
    assert!(!session.workspace.may_close_window());
    assert_eq!(error(&session), Some(unsaved(&dir, "a.md").as_str()));
    assert_eq!(shown(&session), path(&dir, "a.md"));
    session.keys(":w⏎");
    assert!(session.workspace.may_close_window());
}

#[test]
fn qa_se_niega_y_qa_bang_cierra() {
    let (dir, mut session) = three_files();
    session.keys("x");
    show(&mut session, &path(&dir, "b.md"));
    session.keys(":qa⏎");
    assert_eq!(session.effect, Effect::Stay);
    assert_eq!(error(&session), Some(unsaved(&dir, "a.md").as_str()));
    session.keys(":qa!⏎");
    assert_eq!(session.effect, Effect::CloseWindow);
}

#[test]
fn wa_guarda_tambien_las_notas_ocultas() {
    let (dir, mut session) = three_files();
    session.keys("x");
    show(&mut session, &path(&dir, "b.md"));
    session.keys("x⌃wv");
    show(&mut session, &path(&dir, "c.md"));
    session.keys(":wa⏎");
    assert_eq!(error(&session), None);
    assert_eq!(fs::read_to_string(path(&dir, "a.md")).unwrap(), "1\na2\n");
    assert_eq!(
        fs::read_to_string(path(&dir, "b.md")).unwrap(),
        "",
        "un buffer vacío es un archivo vacío"
    );
    assert_eq!(fs::read_to_string(path(&dir, "c.md")).unwrap(), "c\n");
    session.keys(":qa⏎");
    assert_eq!(session.effect, Effect::CloseWindow);
}

#[test]
fn wqa_guarda_todo_y_cierra() {
    let (dir, mut session) = three_files();
    session.keys("x");
    show(&mut session, &path(&dir, "b.md"));
    session.keys(":wqa⏎");
    assert_eq!(session.effect, Effect::CloseWindow);
    assert_eq!(fs::read_to_string(path(&dir, "a.md")).unwrap(), "1\na2\n");
}

/// Un buffer sin archivo no se puede guardar: E141, los demás se guardan
/// igual y la ventana no se cierra.
#[test]
fn wqa_con_un_buffer_sin_nombre_guarda_los_demas_y_no_cierra() {
    let dir = TempDir::new().unwrap();
    fs::write(path(&dir, "a.md"), "a\n").unwrap();
    let mut session = Session::unnamed();
    session.keys("iz⎋");
    show(&mut session, &path(&dir, "a.md"));
    session.keys("x:xa⏎");
    assert_eq!(session.effect, Effect::Stay);
    assert_eq!(error(&session), Some("E141: No file name for buffer 1"));
    assert_eq!(fs::read_to_string(path(&dir, "a.md")).unwrap(), "");
    session.keys(":q⏎");
    assert_eq!(
        error(&session),
        Some("E162: No write since last change for buffer \"[No Name]\"")
    );
    assert_eq!(session.text(), "z");
}

/// Lo mismo que nvim 0.12.6 (`hidden`, su default): qué paneles quedan,
/// dónde está el foco y el cursor, el último error y qué buffers siguen
/// con cambios. Las teclas van en la notación de nvim; `ctrl-w w` y no
/// `ctrl-w l`, porque nvim `--clean` divide al otro lado.
#[rstest::rstest]
#[case::e_y_volver("ix<Esc>:e b.md<CR>iy<Esc>:e a.md<CR>")]
#[case::e_bang_con_otra_ruta("jx<Esc>:e! b.md<CR>:e a.md<CR>")]
#[case::e_sin_ruta_con_cambios("ix<Esc>:e<CR>")]
#[case::q_con_oculto("ix<Esc>:e b.md<CR>:q<CR>")]
#[case::q_con_dos_ocultos(":e b.md<CR>ix<Esc>:e a.md<CR>iy<Esc>:e c.md<CR>:q<CR>")]
#[case::q_con_el_propio("ix<Esc>:e b.md<CR>iy<Esc>:q<CR>")]
#[case::qa("ix<Esc>:e b.md<CR>:qa<CR>")]
#[case::wq_con_oculto("ix<Esc>:e b.md<CR>:wq<CR>")]
#[case::q_en_un_split("ix<Esc><C-w>v:e b.md<CR><C-w>w:q<CR>")]
#[case::ultimo_panel_tras_un_split("ix<Esc>:e b.md<CR><C-w>v:e c.md<CR>:q<CR>:q<CR>")]
#[case::only("ix<Esc><C-w>v:e b.md<CR><C-w>o")]
#[case::wa("ix<Esc>:e b.md<CR>iy<Esc>:wa<CR>:e a.md<CR>")]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn como_nvim_con_hidden(#[case] keys: &str) {
    let (dir, mut session) = three_files();
    let typed = keys
        .replace("<Esc>", "⎋")
        .replace("<CR>", "⏎")
        .replace("<C-w>", "⌃w");
    let typed = ["a.md", "b.md", "c.md"].iter().fold(typed, |typed, name| {
        typed.replace(
            &format!(" {name}"),
            &format!(" {}", path(&dir, name).display()),
        )
    });
    session.keys(&typed);
    let nvim = panes_oracle::run(dir.path(), "a.md", keys);
    assert_eq!(session.panes(dir.path()), nvim);
}
