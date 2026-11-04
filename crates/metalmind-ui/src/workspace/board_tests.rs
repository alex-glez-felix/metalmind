use std::fs;

use metalmind_board::{Board, Frame, Point};
use metalmind_vim::Mode;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use crate::document::Document;
use crate::workspace::test_session::Session;

const BOARD: &str = concat!(
    "{\n\t\"nodes\":[\n",
    "\t\t{\"id\":\"a\",\"type\":\"text\",\"x\":0,\"y\":0,\"width\":100,\"height\":50,\"text\":\"uno\\ndos\"},\n",
    "\t\t{\"id\":\"b\",\"type\":\"text\",\"x\":300,\"y\":0,\"width\":100,\"height\":50,\"text\":\"tres\"}\n",
    "\t],\n\t\"edges\":[]\n}\n",
);

fn on_board() -> (TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("idea.canvas");
    fs::write(&path, BOARD).unwrap();
    let session = Session::open(&path);
    (dir, session)
}

impl Session {
    fn box_text(&self, id: &str) -> Option<String> {
        let document = self.workspace.focused_document().unwrap();
        let board = Board::parse(&document.buffer().to_string()).unwrap();
        board.node(id)?.text().map(ToOwned::to_owned)
    }

    fn open_text(&mut self, id: &str) {
        let pane = self.pane();
        assert!(self.workspace.open_board_text(pane, id).unwrap());
    }

    fn document(&self) -> &Document {
        self.workspace.focused_document().unwrap()
    }
}

#[test]
fn abrir_una_caja_deja_el_cursor_en_insert_al_final() {
    let (_dir, mut session) = on_board();
    session.open_text("a");
    assert_eq!(session.mode(), Mode::Insert);
    session.type_text(" más");
    assert_eq!(session.box_text("a").unwrap(), "uno\ndos más");
}

#[test]
fn el_motor_edita_el_texto_de_la_caja_con_sus_motions_y_su_undo() {
    let (_dir, mut session) = on_board();
    session.open_text("a");
    session.keys("⎋ggdd");
    assert_eq!(session.box_text("a").unwrap(), "dos");
    session.keys("u");
    assert_eq!(session.box_text("a").unwrap(), "uno\ndos");
    session.keys("A!⎋");
    assert_eq!(session.box_text("a").unwrap(), "uno!\ndos");
    assert_eq!(session.mode(), Mode::Normal);
}

#[test]
fn los_dos_puntos_en_una_caja_alcanzan_solo_su_texto() {
    let (_dir, mut session) = on_board();
    session.open_text("a");
    session.keys("⎋:%d⏎");
    assert_eq!(session.box_text("a").unwrap(), "");
    assert_eq!(session.box_text("b").unwrap(), "tres");
}

#[test]
fn cerrar_la_caja_deja_un_solo_paso_y_vuelve_a_la_pizarra() {
    let (_dir, mut session) = on_board();
    let pane = session.pane();
    session.open_text("b");
    session.type_text(" y cuatro");
    session.keys("⎋");
    session.keys("A y cinco⎋");
    let written = session.text();
    session.workspace.close_board_text(pane).unwrap();
    assert!(session.document().open_board_text().is_none());
    assert_eq!(session.text(), written);
    session.workspace.undo_board(pane).unwrap();
    assert_eq!(format!("{}\n", session.text()), BOARD);
    session.workspace.redo_board(pane).unwrap();
    assert_eq!(session.text(), written);
}

#[test]
fn deshacer_sin_nada_que_deshacer_avisa_y_no_cambia_nada() {
    let (_dir, mut session) = on_board();
    let pane = session.pane();
    session.workspace.undo_board(pane).unwrap();
    session.workspace.redo_board(pane).unwrap();
    assert_eq!(format!("{}\n", session.text()), BOARD);
    assert!(!session.is_modified());
}

#[test]
fn deshacer_cierra_la_caja_abierta_antes() {
    let (_dir, mut session) = on_board();
    let pane = session.pane();
    session.open_text("b");
    session.type_text("!");
    session.workspace.undo_board(pane).unwrap();
    assert!(session.document().open_board_text().is_none());
    assert_eq!(format!("{}\n", session.text()), BOARD);
}

#[test]
fn cambiar_de_panel_o_de_archivo_cierra_la_caja_sin_perder_lo_escrito() {
    let (dir, mut session) = on_board();
    session.open_text("b");
    session.type_text(" escrito");
    session.keys("⎋");
    session.keys(&format!(":e {}⏎", dir.path().join("otra.md").display()));
    assert!(session.file().ends_with("otra.md"));
    session.keys(&format!(":e {}⏎", dir.path().join("idea.canvas").display()));
    assert!(session.file().ends_with("idea.canvas"));
    assert!(session.document().open_board_text().is_none());
    assert_eq!(session.box_text("b").unwrap(), "tres escrito");
}

#[test]
fn salir_de_una_caja_no_deja_saltos_que_rompan_el_motor() {
    let (_dir, mut session) = on_board();
    let pane = session.pane();
    session.open_text("a");
    session.keys("⎋ggG");
    session.workspace.close_board_text(pane).unwrap();
    session.open_text("b");
    session.keys("⎋");
    session.keys("\u{f}");
    session.keys("``");
    session.workspace.close_board_text(pane).unwrap();
    assert_eq!(session.box_text("b").unwrap(), "tres");
}

#[test]
fn el_motor_y_lo_que_edita_vuelven_al_inicio_al_salir() {
    let (_dir, mut session) = on_board();
    let pane = session.pane();
    session.open_text("a");
    session.type_text("texto largo que deja el cursor lejos");
    session.workspace.close_board_text(pane).unwrap();
    assert_eq!(
        session.workspace.view(pane).unwrap().selection.head.get(),
        0
    );
    assert_eq!(session.mode(), Mode::Normal);
    let (_, buffer) = session.document().engine_view();
    assert!(buffer.is_empty());
}

#[test]
fn lo_que_cambia_en_disco_con_una_caja_abierta_se_relee_al_cerrarla() {
    let (dir, mut session) = on_board();
    let pane = session.pane();
    session.open_text("b");
    let path = dir.path().join("idea.canvas");
    let changed = BOARD.replace("tres", "de fuera");
    fs::write(&path, &changed).unwrap();
    session.workspace.file_changed(&path);
    assert!(session.document().open_board_text().is_some());
    session.workspace.close_board_text(pane).unwrap();
    assert_eq!(session.box_text("b").unwrap(), "de fuera");
}

#[test]
fn un_texto_suelto_nuevo_abre_escribiendo_y_vacio_no_se_queda() {
    let (_dir, mut session) = on_board();
    let pane = session.pane();
    let frame = Frame::centered_at(Point::new(0.0, 500.0), (200, 40));
    session.workspace.open_new_board_text(pane, frame).unwrap();
    assert_eq!(session.mode(), Mode::Insert);
    session.workspace.close_board_text(pane).unwrap();
    assert_eq!(format!("{}\n", session.text()), BOARD);
    assert!(!session.is_modified());
}

#[test]
fn el_autoguardado_escribe_lo_de_una_caja_abierta_y_lo_que_mueve_la_pizarra() {
    use std::rc::Rc;
    use std::time::Duration;

    use crate::workspace::{Autosave, SystemClock};

    let (dir, mut session) = on_board();
    let pane = session.pane();
    session
        .workspace
        .set_autosave(Some(Autosave::new(Duration::ZERO, Rc::new(SystemClock))));
    let path = dir.path().join("idea.canvas");
    session
        .workspace
        .edit_board(pane, |board| {
            board.move_nodes(&["a".to_owned()], 10, 0);
        })
        .unwrap();
    session.workspace.autosave_all();
    let moved = fs::read_to_string(&path).unwrap();
    assert!(
        moved.contains(r#""id":"a","type":"text","x":10"#),
        "{moved}"
    );
    assert!(moved.ends_with("}\n"));
    session.open_text("b");
    session.type_text(" a medias");
    session.workspace.autosave_all();
    assert!(fs::read_to_string(&path).unwrap().contains("tres a medias"));
    session.workspace.close_board_text(pane).unwrap();
    assert!(!session.is_modified());
    session.workspace.undo_board(pane).unwrap();
    assert!(session.is_modified());
    assert!(session.text().contains(r#""text":"tres""#));
}
