use std::path::Path;

use metalmind_board::{Corner, Frame, Point, Side};
use metalmind_buffer::Edit;
use pretty_assertions::assert_eq;

use super::*;
use crate::disk::Files;
use crate::document::Editable;

const BOARD: &str = concat!(
    "{\n\t\"nodes\":[\n",
    "\t\t{\"id\":\"a\",\"type\":\"text\",\"x\":0,\"y\":0,\"width\":100,\"height\":50,\"text\":\"uno\",\"x-plugin\":{\"k\":[1,2]}},\n",
    "\t\t{\"id\":\"b\",\"type\":\"text\",\"x\":300,\"y\":0,\"width\":100,\"height\":50,\"text\":\"dos\"},\n",
    "\t\t{\"id\":\"c\",\"type\":\"text\",\"x\":600,\"y\":0,\"width\":100,\"height\":50,\"shape\":\"none\",\"text\":\"suelto\"}\n",
    "\t],\n\t\"edges\":[\n",
    "\t\t{\"id\":\"ab\",\"fromNode\":\"a\",\"toNode\":\"b\",\"x-plugin\":true},\n",
    "\t\t{\"id\":\"bc\",\"fromNode\":\"b\",\"toNode\":\"c\"}\n",
    "\t]\n}",
);

fn document() -> Document {
    Document::new(Path::new("idea.canvas"), BOARD)
}

fn ids(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn text(document: &Document) -> String {
    document.buffer().to_string()
}

/// Las líneas de `after` que `before` no tiene y las de `before` que `after` no.
fn diff(before: &str, after: &str) -> (Vec<String>, Vec<String>) {
    let only_in = |a: &str, b: &str| -> Vec<String> {
        let others: Vec<&str> = b.lines().collect();
        a.lines()
            .filter(|line| !others.contains(line))
            .map(ToOwned::to_owned)
            .collect()
    };
    (only_in(before, after), only_in(after, before))
}

/// Tipear en la caja abierta como lo haría el motor.
fn type_in_scratch(document: &mut Document, typed: &str) {
    let Editable { buffer, .. } = document.editable();
    buffer.begin_transaction(at_start()).unwrap();
    let end = buffer.len();
    buffer.apply(vec![Edit::insert(end, typed)]).unwrap();
    buffer.commit_transaction(at_start()).unwrap();
    document.sync_board_text().unwrap();
}

type BoardEdit = fn(&mut Board);

/// Cada edición, para los tests que deben valer con cualquiera.
fn edits() -> Vec<(&'static str, BoardEdit)> {
    vec![
        ("mover", |board| {
            board.move_nodes(&ids(&["a", "b"]), 10, -20);
        }),
        ("escribir", |board| {
            board.set_text("b", "otro \"texto\"\ncon ñ y 日本語");
        }),
        ("crear", |board| {
            let frame = Frame::centered_at(Point::new(50.0, 500.0), (200, 80));
            board.add_text(frame, Shape::Ellipse, "nueva");
        }),
        ("redimensionar", |board| {
            let node = board.nodes.iter_mut().find(|node| node.id == "a").unwrap();
            let frame = node
                .frame()
                .dragged(Corner::BottomRight, Point::new(250.0, 90.0));
            node.set_frame(frame);
        }),
        ("unir", |board| {
            board.connect("a", "c");
        }),
        ("caja unida", |board| {
            board.add_linked_box("c", Side::Bottom);
        }),
        ("borrar un nodo", |board| board.remove(&ids(&["b"]))),
        ("borrar el último nodo", |board| board.remove(&ids(&["c"]))),
        ("pegar", |board| {
            let copied = board.copy(&ids(&["a", "b"]));
            board.paste(&copied, Point::new(0.0, 0.0));
        }),
    ]
}

#[test]
fn deshacer_y_rehacer_devuelven_los_bytes_exactos() {
    for (name, edit) in edits() {
        let mut document = document();
        document.edit_board(edit).unwrap();
        let edited = text(&document);
        assert_ne!(edited, BOARD, "{name}");
        assert!(document.undo_board().unwrap(), "{name}");
        assert_eq!(text(&document), BOARD, "{name}: deshacer");
        assert!(!document.is_modified(), "{name}: deshacer deja lo guardado");
        assert!(document.redo_board().unwrap(), "{name}");
        assert_eq!(text(&document), edited, "{name}: rehacer");
    }
}

#[test]
fn varias_ediciones_se_deshacen_de_una_en_una_hasta_el_principio() {
    let mut document = document();
    let mut states = vec![text(&document)];
    for (_, edit) in edits() {
        document.edit_board(edit).unwrap();
        states.push(text(&document));
    }
    for state in states.iter().rev().skip(1) {
        assert!(document.undo_board().unwrap());
        assert_eq!(&text(&document), state);
    }
    assert!(!document.undo_board().unwrap());
    for state in states.iter().skip(1) {
        assert!(document.redo_board().unwrap());
        assert_eq!(&text(&document), state);
    }
    assert!(!document.redo_board().unwrap());
}

#[test]
fn una_edicion_no_toca_las_lineas_de_otros_elementos() {
    let mut document = document();
    document
        .edit_board(|board| board.move_nodes(&ids(&["a"]), 10, 0))
        .unwrap();
    let (removed, added) = diff(BOARD, &text(&document));
    assert_eq!(removed.len(), 1);
    assert!(removed[0].contains("\"id\":\"a\""));
    assert_eq!(added.len(), 1);
    assert!(added[0].contains("\"id\":\"a\""));
    assert!(added[0].contains("\"x\":10"));
}

#[test]
fn borrar_toca_las_lineas_de_lo_borrado_y_las_flechas_que_se_van_con_ello() {
    let mut document = document();
    document
        .edit_board(|board| board.remove(&ids(&["a"])))
        .unwrap();
    let (removed, added) = diff(BOARD, &text(&document));
    assert_eq!(added, Vec::<String>::new());
    assert_eq!(removed.len(), 2);
    assert!(removed[0].contains("\"id\":\"a\""));
    assert!(removed[1].contains("\"id\":\"ab\""));
}

#[test]
fn crear_toca_la_coma_de_la_linea_anterior_y_nada_mas() {
    let mut document = document();
    document
        .edit_board(|board| {
            let frame = Frame::centered_at(Point::new(0.0, 500.0), (200, 80));
            board.add_text(frame, Shape::Rectangle, "nueva");
        })
        .unwrap();
    let (removed, added) = diff(BOARD, &text(&document));
    assert_eq!(removed.len(), 1);
    assert!(removed[0].contains("\"id\":\"c\""));
    assert_eq!(added.len(), 2);
    assert_eq!(added[0], format!("{},", removed[0]));
}

#[test]
fn las_claves_desconocidas_siguen_tras_cualquier_edicion() {
    for (name, edit) in edits() {
        let mut document = document();
        document.edit_board(edit).unwrap();
        let edited = text(&document);
        if name != "borrar un nodo" {
            assert!(edited.contains(r#""x-plugin":{"k":[1,2]}"#), "{name}");
            assert!(edited.contains(r#""x-plugin":true"#), "{name}");
        }
    }
}

#[test]
fn una_edicion_sin_efecto_no_deja_paso_ni_toca_un_archivo_no_canonico() {
    let spaced = "{\"nodes\": [ {\"id\":\"a\",\"type\":\"text\",\"x\":0,\"y\":0,\"width\":1,\"height\":1,\"text\":\"\"} ]}";
    let mut document = Document::new(Path::new("x.canvas"), spaced);
    document
        .edit_board(|board| board.move_nodes(&ids(&["no-existe"]), 10, 0))
        .unwrap();
    assert_eq!(text(&document), spaced);
    assert!(!document.undo_board().unwrap());
}

#[test]
fn un_archivo_que_no_se_lee_no_se_edita() {
    let broken = "{\"nodes\": [";
    let mut document = Document::new(Path::new("x.canvas"), broken);
    let edited = document.edit_board(|board| board.remove(&ids(&["a"])));
    assert!(matches!(edited, Err(BoardEditError::Format(_))));
    assert!(matches!(
        document.begin_board_text("a"),
        Err(BoardEditError::Format(_))
    ));
    assert_eq!(text(&document), broken);
    assert!(!document.is_modified());
}

#[test]
fn salir_de_una_caja_es_un_solo_paso_aunque_se_haya_tecleado_mucho() {
    let mut document = document();
    assert!(document.begin_board_text("b").unwrap());
    assert_eq!(
        document.open_board_text().unwrap().buffer.to_string(),
        "dos"
    );
    for typed in [" y", " tres", " más"] {
        type_in_scratch(&mut document, typed);
    }
    let written = text(&document);
    assert!(written.contains(r#""text":"dos y tres más""#));
    document.end_board_text().unwrap();
    assert_eq!(document.open_board_text().map(|open| open.element), None);
    assert_eq!(text(&document), written);
    assert!(document.undo_board().unwrap());
    assert_eq!(text(&document), BOARD);
    assert!(document.redo_board().unwrap());
    assert_eq!(text(&document), written);
}

#[test]
fn lo_escrito_llega_al_archivo_mientras_se_escribe_y_se_guarda_a_medias() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("idea.canvas");
    std::fs::write(&path, format!("{BOARD}\n")).unwrap();
    let mut document = Document::open(&path).unwrap();
    document.begin_board_text("b").unwrap();
    type_in_scratch(&mut document, " y más");
    assert!(document.is_modified());
    document
        .save(&Files, metalmind_vault::SaveKind::Explicit)
        .unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("dos y más")
    );
    document.end_board_text().unwrap();
    assert!(!document.is_modified());
    assert!(document.undo_board().unwrap());
    assert_eq!(text(&document), BOARD);
    assert!(document.is_modified());
}

#[test]
fn abrir_y_cerrar_una_caja_sin_escribir_no_deja_paso() {
    let mut document = document();
    document.begin_board_text("a").unwrap();
    document.end_board_text().unwrap();
    assert_eq!(text(&document), BOARD);
    assert!(!document.is_modified());
    assert!(!document.undo_board().unwrap());
}

#[test]
fn la_etiqueta_de_una_flecha_se_escribe_como_el_texto_de_una_caja() {
    let mut document = document();
    assert!(document.begin_board_text("bc").unwrap());
    type_in_scratch(&mut document, "lleva a");
    document.end_board_text().unwrap();
    assert!(text(&document).contains(r#""toNode":"c","label":"lleva a""#));
}

#[test]
fn una_tarjeta_no_abre_texto() {
    let card = "{\"nodes\":[{\"id\":\"t\",\"type\":\"file\",\"x\":0,\"y\":0,\"width\":1,\"height\":1,\"file\":\"n.md\"}]}";
    let mut document = Document::new(Path::new("x.canvas"), card);
    assert!(!document.begin_board_text("t").unwrap());
    assert!(!document.buffer().has_open_transaction());
}

#[test]
fn un_texto_suelto_nuevo_y_vacio_no_llega_a_existir() {
    let mut document = document();
    let frame = Frame::centered_at(Point::new(0.0, 500.0), (200, 40));
    document.begin_new_board_text(frame).unwrap();
    assert_ne!(text(&document), BOARD);
    document.end_board_text().unwrap();
    assert_eq!(text(&document), BOARD);
    assert!(!document.is_modified());
}

#[test]
fn un_texto_suelto_nuevo_con_algo_escrito_es_un_solo_paso() {
    let mut document = document();
    let frame = Frame::centered_at(Point::new(0.0, 500.0), (200, 40));
    let id = document.begin_new_board_text(frame).unwrap();
    type_in_scratch(&mut document, "hola");
    document.end_board_text().unwrap();
    assert!(text(&document).contains(&format!(r#""id":"{id}""#)));
    assert!(document.undo_board().unwrap());
    assert_eq!(text(&document), BOARD);
}

#[test]
fn vaciar_un_texto_suelto_que_ya_estaba_lo_borra_y_se_puede_deshacer() {
    let mut document = document();
    document.begin_board_text("c").unwrap();
    {
        let Editable { buffer, .. } = document.editable();
        buffer.begin_transaction(at_start()).unwrap();
        let end = buffer.len();
        buffer
            .apply(vec![Edit::delete(ByteOffset::ZERO..end)])
            .unwrap();
        buffer.commit_transaction(at_start()).unwrap();
    }
    document.sync_board_text().unwrap();
    document.end_board_text().unwrap();
    assert!(!text(&document).contains("\"id\":\"c\""));
    assert!(document.undo_board().unwrap());
    assert_eq!(text(&document), BOARD);
}

#[test]
fn vaciar_una_caja_no_la_borra() {
    let mut document = document();
    document.begin_board_text("b").unwrap();
    {
        let Editable { buffer, .. } = document.editable();
        buffer.begin_transaction(at_start()).unwrap();
        let end = buffer.len();
        buffer
            .apply(vec![Edit::delete(ByteOffset::ZERO..end)])
            .unwrap();
        buffer.commit_transaction(at_start()).unwrap();
    }
    document.sync_board_text().unwrap();
    document.end_board_text().unwrap();
    assert!(text(&document).contains(r#""id":"b""#));
}
