use std::fs;
use std::path::{Path, PathBuf};

use metalmind_buffer::ByteOffset;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::workspace::test_session::Session;
use crate::workspace::{LineNumbers, panes_oracle};

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
    session
        .view(session.pane())
        .document
        .path()
        .unwrap()
        .to_owned()
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
    // Como vim con `hidden`, a.md queda oculto.
    assert_eq!(session.workspace.documents.iter().count(), 2);
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

/// Con `hidden`, el default de nvim: el buffer con cambios queda oculto.
#[test]
fn e_deja_oculto_el_buffer_con_cambios() {
    let (dir, mut session) = two_files();
    session.keys("x");
    edit(&mut session, "e", &path(&dir, "b.md"));
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(error(&session), None);
    assert_eq!(session.workspace.documents.iter().count(), 2);
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.text(), "");
    assert!(session.is_modified());
}

/// `:e!` con otra ruta no descarta nada en nvim con `hidden`: solo `:e!`
/// sin ruta relee.
#[test]
fn e_con_bang_y_otra_ruta_tambien_deja_oculto() {
    let (dir, mut session) = two_files();
    session.keys("x");
    edit(&mut session, "e!", &path(&dir, "b.md"));
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(session.workspace.documents.iter().count(), 2);
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.text(), "");
    session.keys(":e!⏎");
    assert_eq!(session.text(), "a");
    assert!(!session.is_modified());
}

/// Como en nvim, también sin cambios: al volver, el cursor y el historial
/// siguen ahí.
#[test]
fn e_deja_oculto_el_buffer_sin_cambios() {
    let (dir, _) = two_files();
    fs::write(path(&dir, "a.md"), "a1\na2\n").unwrap();
    let mut session = Session::open(&path(&dir, "a.md"));
    session.keys("jx:w⏎");
    edit(&mut session, "e", &path(&dir, "b.md"));
    assert_eq!(session.workspace.documents.iter().count(), 2);
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.cursor(), ByteOffset::new(3));
    session.keys("u");
    assert_eq!(session.text(), "a1\na2");
}

/// Si el archivo cambió fuera de Metalmind mientras el buffer estaba oculto y
/// sin cambios, vuelve con lo del disco, como con `autoread`; el cursor se
/// queda en su línea.
#[test]
fn volver_a_un_buffer_oculto_sin_cambios_relee_lo_que_cambio_fuera() {
    let (dir, _) = two_files();
    fs::write(path(&dir, "a.md"), "a1\na2\n").unwrap();
    let mut session = Session::open(&path(&dir, "a.md"));
    session.keys("j");
    edit(&mut session, "e", &path(&dir, "b.md"));
    fs::write(path(&dir, "a.md"), "x1\nx2\nx3\n").unwrap();
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.text(), "x1\nx2\nx3");
    assert_eq!(session.cursor(), ByteOffset::new(3));
    assert!(!session.is_modified());
}

/// Un archivo que no existía al abrirlo y que otro programa creó mientras
/// el buffer estaba oculto: vuelve con lo de fuera, y un `:w` no lo pisa.
#[test]
fn volver_a_un_buffer_oculto_carga_el_archivo_que_aparecio() {
    let (dir, mut session) = two_files();
    edit(&mut session, "e", &path(&dir, "nueva.md"));
    edit(&mut session, "e", &path(&dir, "b.md"));
    fs::write(path(&dir, "nueva.md"), "de fuera\n").unwrap();
    edit(&mut session, "e", &path(&dir, "nueva.md"));
    assert_eq!(session.text(), "de fuera");
    session.keys(":w⏎");
    assert_eq!(
        fs::read_to_string(path(&dir, "nueva.md")).unwrap(),
        "de fuera\n"
    );
}

/// El texto del buffer no distingue un archivo vacío de una línea vacía:
/// si solo cambió eso, también se relee, y `:w` escribe lo de fuera.
#[test]
fn volver_a_un_buffer_oculto_relee_un_salto_de_linea_que_aparecio() {
    let (dir, _) = two_files();
    fs::write(path(&dir, "a.md"), "").unwrap();
    let mut session = Session::open(&path(&dir, "a.md"));
    edit(&mut session, "e", &path(&dir, "b.md"));
    fs::write(path(&dir, "a.md"), "\n").unwrap();
    edit(&mut session, "e", &path(&dir, "a.md"));
    session.keys(":w⏎");
    assert_eq!(fs::read_to_string(path(&dir, "a.md")).unwrap(), "\n");
}

/// Si lo borraron fuera, el buffer oculto es la única copia: vuelve igual,
/// con su historial.
#[test]
fn volver_a_un_buffer_oculto_cuyo_archivo_borraron_lo_conserva() {
    let (dir, _) = two_files();
    let mut session = Session::open(&path(&dir, "a.md"));
    session.keys("ix⎋:w⏎");
    edit(&mut session, "e", &path(&dir, "b.md"));
    fs::remove_file(path(&dir, "a.md")).unwrap();
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.text(), "xa");
    session.keys("u");
    assert_eq!(session.text(), "a");
}

/// Guardar una nota que era una sola línea vacía la deja como lo que dice
/// el disco: al volver no parece cambiada fuera y conserva su historial.
#[test]
fn volver_a_una_nota_guardada_que_era_una_linea_vacia_conserva_el_historial() {
    let (dir, _) = two_files();
    fs::write(path(&dir, "a.md"), "\n").unwrap();
    let mut session = Session::open(&path(&dir, "a.md"));
    session.keys("ix⎋:w⏎");
    edit(&mut session, "e", &path(&dir, "b.md"));
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.text(), "x");
    session.keys("u");
    assert_eq!(session.text(), "", "el undo sigue ahí");
}

/// Lo mismo si vuelve en un split.
#[test]
fn abrir_en_split_un_buffer_oculto_sin_cambios_relee_lo_que_cambio_fuera() {
    let (dir, mut session) = two_files();
    edit(&mut session, "e", &path(&dir, "b.md"));
    fs::write(path(&dir, "a.md"), "de fuera\n").unwrap();
    session
        .workspace
        .open_in_split(Axis::Row, &path(&dir, "a.md"))
        .unwrap();
    assert_eq!(session.text(), "de fuera");
}

/// Con cambios, lo de fuera no pisa el buffer.
#[test]
fn volver_a_un_buffer_oculto_con_cambios_no_relee() {
    let (dir, mut session) = two_files();
    session.keys("x");
    edit(&mut session, "e", &path(&dir, "b.md"));
    fs::write(path(&dir, "a.md"), "de fuera\n").unwrap();
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert_eq!(session.text(), "");
    assert!(session.is_modified());
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
    assert_eq!(session.workspace.documents.iter().count(), 2);
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
        Some("E162: No write since last change for buffer \"prueba.md\"")
    );
    session.keys(":q!⏎");
    assert_eq!(session.effect, Effect::CloseWindow);
}

#[test]
fn q_cierra_el_panel_y_sus_cambios_quedan_ocultos() {
    let (dir, mut session) = two_files();
    session.keys("⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("x:q⏎");
    assert_eq!(session.workspace.tree().len(), 1);
    assert_eq!(error(&session), None);
    assert_eq!(shown(&session), path(&dir, "a.md"));
    assert_eq!(session.workspace.documents.iter().count(), 2);
    session.keys(":q⏎");
    assert_eq!(session.effect, Effect::Stay);
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(session.text(), "1\nb2");
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
    let right = session.pane();
    assert!(!session.workspace.may_close_window());
    let unsaved = format!(
        "E162: No write since last change for buffer \"{}\"",
        path(&dir, "a.md").display()
    );
    assert_eq!(error(&session), Some(unsaved.as_str()));
    assert_ne!(session.pane(), right, "va al panel con cambios");
    assert_eq!(shown(&session), path(&dir, "a.md"));
    session.keys(":w⏎");
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
fn only_cierra_los_demas_y_sus_cambios_quedan_ocultos() {
    let (dir, mut session) = two_files();
    session.keys("⌃wv⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("x⌃wv");
    let keep = session.pane();
    edit(&mut session, "e", &path(&dir, "nueva.md"));
    session.keys("⌃wo");
    assert_eq!(session.workspace.tree().len(), 1);
    assert_eq!(session.pane(), keep);
    assert_eq!(error(&session), None);
    assert_eq!(session.workspace.documents.iter().count(), 3);
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
        Some("E37: No write since last change (add ! to override)")
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

#[test]
fn space_f_y_space_p_piden_el_finder_y_la_paleta() {
    let mut session = Session::new("uno\n");
    session.keys(" f");
    assert_eq!(session.effect, Effect::OpenFinder);
    session.keys(" p");
    assert_eq!(session.effect, Effect::OpenCommandPalette);
    session.keys("ix⎋");
    assert_eq!(session.text(), "xuno\n", "space no deja nada pendiente");
}

/// El picker se lleva las teclas: lo que quede de la macro no sigue.
#[test]
fn abrir_el_finder_corta_la_macro() {
    let mut session = Session::new("uno\n");
    session.keys("qa f");
    session.keys("⎋q");
    session.keys("ix⎋");
    session.keys("@a");
    assert_eq!(session.effect, Effect::OpenFinder);
    assert_eq!(session.workspace.vim.recording(), None);
}

#[test]
fn open_in_split_abre_otro_archivo_en_un_panel_nuevo() {
    let (dir, mut session) = two_files();
    let before = session.pane();
    session
        .workspace
        .open_in_split(Axis::Row, &path(&dir, "b.md"))
        .unwrap();
    assert_eq!(session.workspace.tree().len(), 2);
    assert_ne!(session.pane(), before);
    assert_eq!(shown(&session), path(&dir, "b.md"));
    assert_eq!(
        session.view(before).document.path(),
        Some(path(&dir, "a.md").as_path())
    );
}

/// Abrir en split la nota del panel, con cambios, la comparte: no relee ni
/// se niega como `:e` del mismo archivo.
#[test]
fn open_in_split_de_la_nota_del_panel_la_comparte_aunque_tenga_cambios() {
    let (dir, mut session) = two_files();
    session.keys("ix⎋");
    session
        .workspace
        .open_in_split(Axis::Column, &path(&dir, "a.md"))
        .unwrap();
    assert_eq!(session.workspace.tree().len(), 2);
    assert_eq!(error(&session), None);
    assert_eq!(session.text(), "xa");
}

#[test]
fn open_in_split_de_algo_ilegible_no_divide() {
    let (dir, mut session) = two_files();
    fs::write(path(&dir, "binario.md"), [0xff, 0xfe]).unwrap();
    session
        .workspace
        .open_in_split(Axis::Row, &path(&dir, "binario.md"))
        .unwrap();
    assert_eq!(session.workspace.tree().len(), 1);
    assert!(error(&session).is_some());
}

/// Abrir desde el finder con un modo a medias (Insert, por ejemplo con la
/// paleta abierta desde Insert): el panel de antes termina lo pendiente
/// como un `ctrl-c`, igual que al cambiar de panel, y el nuevo empieza en
/// Normal. En nvim es lo de teclear `<C-c>` y después el comando; con
/// `<C-o>:vsplit` o `<Cmd>vsplit` nvim seguiría en Insert, pero Metalmind nunca
/// cambia de panel sin salir de lo pendiente (`Workspace::focus`).
#[rstest::rstest]
#[case::split_desde_insert("Aho", Opened::Split)]
#[case::split_desde_un_operador("d", Opened::Split)]
#[case::split_desde_la_linea_de_comandos(":ab", Opened::Split)]
#[case::edit_desde_insert_con_cambios("Aho", Opened::Edit)]
#[case::edit_desde_la_linea_de_comandos(":ab", Opened::Edit)]
#[cfg_attr(not(nvim_available), ignore = "nvim no está en el PATH")]
fn abrir_desde_un_modo_a_medias_es_como_nvim_tras_ctrl_c(
    #[case] pending: &str,
    #[case] opened: Opened,
) {
    let (dir, mut session) = two_files();
    session.keys(pending);
    assert_ne!(session.mode(), metalmind_vim::Mode::Normal);
    let target = path(&dir, "b.md");
    let command = match opened {
        Opened::Split => {
            session.workspace.open_in_split(Axis::Row, &target).unwrap();
            "vsplit"
        }
        Opened::Edit => {
            let edit = metalmind_vim::Invocation {
                command: metalmind_vim::Command::Edit,
                argument: Some(target.display().to_string()),
            };
            session.effect = session.workspace.run(&edit, &mut session.clipboard);
            "edit"
        }
    };
    let nvim = panes_oracle::run(
        dir.path(),
        "a.md",
        &format!("{pending}<C-c>:{command} b.md<CR>"),
    );
    assert_eq!(session.panes(dir.path()), nvim);
    assert_eq!(nvim.mode, "n");
}

#[derive(Clone, Copy, Debug)]
enum Opened {
    Split,
    Edit,
}

#[test]
fn space_e_y_space_t_le_piden_el_panel_lateral_a_la_ventana() {
    let mut session = Session::new("texto");
    session.keys(" e");
    assert_eq!(session.effect, Effect::TogglePanel(PanelView::Folders));
    session.keys(" t");
    assert_eq!(session.effect, Effect::TogglePanel(PanelView::Tags));
    assert_eq!(session.cursor(), ByteOffset::ZERO, "space no se movió");
}

/// Sin vecino a la izquierda, `ctrl-w h` le deja el foco a lo que haya
/// fuera de los paneles, como el panel lateral.
#[test]
fn ctrl_w_hacia_el_borde_sale_de_los_paneles() {
    let mut session = Session::new("texto");
    session.keys("⌃wh");
    assert_eq!(session.effect, Effect::FocusPastEdge(Direction::Left));
    session.keys("⌃wv");
    let right = session.pane();
    session.keys("⌃wh");
    assert_eq!(session.effect, Effect::Stay);
    assert_ne!(session.pane(), right);
    session.keys("⌃wk");
    assert_eq!(session.effect, Effect::FocusPastEdge(Direction::Up));
}

/// Al abrir una carpeta, el buffer `[No Name]` se guarda con `:w ruta`.
#[test]
fn w_con_ruta_le_da_nombre_al_buffer_sin_nombre() {
    let dir = TempDir::new().unwrap();
    let mut session = Session::unnamed();
    session.keys("ihola⎋:w⏎");
    assert_eq!(error(&session), Some("E32: No file name"));
    edit(&mut session, "w", &path(&dir, "nueva.md"));
    assert_eq!(shown(&session), path(&dir, "nueva.md"));
    assert_eq!(
        fs::read_to_string(path(&dir, "nueva.md")).unwrap(),
        "hola\n"
    );
    edit(&mut session, "w", &path(&dir, "nueva.md"));
    assert_eq!(error(&session), None, ":w de su propio archivo guarda");
    session.keys("0ix⎋");
    edit(&mut session, "wq", &path(&dir, "nueva.md"));
    assert_eq!(session.effect, Effect::CloseWindow);
    assert_eq!(
        fs::read_to_string(path(&dir, "nueva.md")).unwrap(),
        "xhola\n"
    );
}

#[test]
fn w_con_otra_ruta_no_pisa_un_archivo() {
    let (dir, mut session) = two_files();
    edit(&mut session, "w", &path(&dir, "b.md"));
    assert_eq!(
        error(&session),
        Some("E13: File exists (add ! to override)")
    );
    assert_eq!(fs::read_to_string(path(&dir, "b.md")).unwrap(), "b1\nb2\n");
    edit(&mut session, "w", &path(&dir, "copia.md"));
    assert_eq!(fs::read_to_string(path(&dir, "copia.md")).unwrap(), "a\n");
    assert_eq!(shown(&session), path(&dir, "a.md"));
}

/// nvim 0.12.6: `:wq otro.md` con cambios escribe la copia y no cierra,
/// porque el buffer sigue con cambios; sin cambios, escribe y cierra.
#[test]
fn wq_con_otra_ruta_y_cambios_escribe_la_copia_y_no_cierra() {
    let (dir, mut session) = two_files();
    session.keys("x");
    edit(&mut session, "wq", &path(&dir, "copia.md"));
    assert_eq!(fs::read_to_string(path(&dir, "copia.md")).unwrap(), "");
    assert_eq!(session.effect, Effect::Stay);
    let unsaved = format!(
        "E162: No write since last change for buffer \"{}\"",
        path(&dir, "a.md").display()
    );
    assert_eq!(error(&session), Some(unsaved.as_str()));
    session.keys(":e!⏎");
    edit(&mut session, "wq", &path(&dir, "otra.md"));
    assert_eq!(session.effect, Effect::CloseWindow);
}

/// En un split, el panel se cierra y el buffer queda oculto con sus cambios.
#[test]
fn wq_con_otra_ruta_en_un_split_deja_oculto_el_buffer() {
    let (dir, mut session) = two_files();
    session.keys("x⌃wv");
    edit(&mut session, "e", &path(&dir, "b.md"));
    session.keys("⌃wh");
    edit(&mut session, "wq", &path(&dir, "copia.md"));
    assert_eq!(error(&session), None);
    assert_eq!(session.workspace.tree().len(), 1);
    edit(&mut session, "e", &path(&dir, "a.md"));
    assert!(session.is_modified());
}

/// nvim 0.12.6: `:e nueva.md`, `:enew` y `:w nueva.md` dan E139 si
/// `nueva.md` no existe; si existe, E13.
#[test]
fn w_a_la_ruta_de_otro_buffer_abierto_da_e139() {
    let (dir, mut session) = two_files();
    edit(&mut session, "e", &path(&dir, "nueva.md"));
    session.keys("⌃wv");
    edit(&mut session, "e!", &path(&dir, "a.md"));
    session.keys("ix⎋");
    edit(&mut session, "w", &path(&dir, "nueva.md"));
    assert_eq!(
        error(&session),
        Some("E139: File is loaded in another buffer")
    );
    assert!(!path(&dir, "nueva.md").exists());
    edit(&mut session, "wq", &path(&dir, "nueva.md"));
    assert_eq!(
        error(&session),
        Some("E139: File is loaded in another buffer")
    );
    assert_eq!(session.effect, Effect::Stay);
    edit(&mut session, "w", &path(&dir, "b.md"));
    assert_eq!(
        error(&session),
        Some("E13: File exists (add ! to override)")
    );
}

#[test]
fn w_desde_un_buffer_sin_nombre_a_una_nota_abierta_da_e139() {
    let dir = TempDir::new().unwrap();
    let mut session = Session::unnamed();
    session.keys("ihola⎋⌃wv");
    edit(&mut session, "e!", &path(&dir, "nueva.md"));
    session.keys("⌃wh");
    edit(&mut session, "w", &path(&dir, "nueva.md"));
    assert_eq!(
        error(&session),
        Some("E139: File is loaded in another buffer")
    );
    assert_eq!(session.view(session.pane()).document.path(), None);
}
