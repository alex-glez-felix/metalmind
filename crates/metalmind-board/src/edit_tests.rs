use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;

const BOARD: &str = r#"{"nodes":[
    {"id":"a","type":"text","x":0,"y":0,"width":100,"height":50,"text":"uno","x-plugin":1},
    {"id":"b","type":"text","x":300,"y":0,"width":100,"height":50,"text":"dos"},
    {"id":"c","type":"text","x":600,"y":0,"width":100,"height":50,"text":"tres"}
],"edges":[
    {"id":"ab","fromNode":"a","toNode":"b","x-plugin":2},
    {"id":"bc","fromNode":"b","toNode":"c"}
]}"#;

fn board() -> Board {
    Board::parse(BOARD).unwrap()
}

fn ids(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// Las líneas del archivo que difieren de las del tablero sin tocar.
fn changed_lines(after: &Board) -> Vec<String> {
    let before = board().write();
    let before: Vec<&str> = before.lines().collect();
    after
        .write()
        .lines()
        .filter(|line| !before.contains(line))
        .map(ToOwned::to_owned)
        .collect()
}

#[rstest]
#[case(14.0, 10)]
#[case(15.0, 20)]
#[case(-14.0, -10)]
#[case(-16.0, -20)]
fn la_rejilla_redondea_al_mas_cercano(#[case] value: f32, #[case] expected: i32) {
    assert_eq!(snap(value), expected);
}

#[test]
fn dibujar_da_una_caja_en_la_rejilla_en_cualquier_sentido() {
    let frame = Frame::between(Point::new(103.0, 58.0), Point::new(-12.0, 7.0));
    assert_eq!(
        frame,
        Frame {
            x: -10,
            y: 10,
            width: 110,
            height: 50
        }
    );
}

#[test]
fn dibujar_sin_mover_no_da_una_caja_vacia() {
    let frame = Frame::between(Point::new(3.0, 3.0), Point::new(3.0, 3.0));
    assert_eq!((frame.width, frame.height), (GRID, GRID));
}

#[rstest]
#[case(Corner::BottomRight, 150.0, 90.0, Frame { x: 0, y: 0, width: 150, height: 90 })]
#[case(Corner::TopLeft, -30.0, -20.0, Frame { x: -30, y: -20, width: 130, height: 70 })]
#[case(Corner::TopRight, 80.0, 10.0, Frame { x: 0, y: 10, width: 80, height: 40 })]
#[case(Corner::BottomLeft, 20.0, 70.0, Frame { x: 20, y: 0, width: 80, height: 70 })]
// Pasarse de la esquina opuesta deja una celda.
#[case(Corner::BottomRight, -500.0, -500.0, Frame { x: 0, y: 0, width: GRID, height: GRID })]
#[case(Corner::TopLeft, 500.0, 500.0, Frame { x: 90, y: 40, width: GRID, height: GRID })]
fn la_esquina_opuesta_no_se_mueve(
    #[case] corner: Corner,
    #[case] x: f32,
    #[case] y: f32,
    #[case] expected: Frame,
) {
    let frame = Frame {
        x: 0,
        y: 0,
        width: 100,
        height: 50,
    };
    assert_eq!(frame.dragged(corner, Point::new(x, y)), expected);
}

#[test]
fn mover_un_nodo_cambia_solo_su_linea_y_conserva_lo_desconocido() {
    let mut board = board();
    board.move_nodes(&ids(&["a"]), 10, 20);
    assert_eq!(
        changed_lines(&board),
        [
            r#"		{"id":"a","type":"text","x":10,"y":20,"width":100,"height":50,"text":"uno","x-plugin":1},"#
        ]
    );
}

#[test]
fn escribir_cambia_solo_la_linea_de_esa_caja() {
    let mut board = board();
    assert!(board.set_text("b", "otro\ntexto"));
    assert_eq!(
        changed_lines(&board),
        [
            r#"		{"id":"b","type":"text","x":300,"y":0,"width":100,"height":50,"text":"otro\ntexto"},"#
        ]
    );
}

#[test]
fn la_etiqueta_vacia_de_una_flecha_no_se_escribe() {
    let mut board = board();
    assert!(board.set_text("bc", "lleva a"));
    assert_eq!(
        changed_lines(&board),
        [r#"		{"id":"bc","fromNode":"b","toNode":"c","label":"lleva a"}"#]
    );
    assert!(board.set_text("bc", ""));
    assert_eq!(changed_lines(&board), Vec::<String>::new());
}

#[test]
fn una_tarjeta_no_tiene_texto_que_escribir() {
    let mut board = Board::parse(
        r#"{"nodes":[{"id":"t","type":"file","x":0,"y":0,"width":9,"height":9,"file":"n.md"}]}"#,
    )
    .unwrap();
    assert!(!board.set_text("t", "x"));
    assert_eq!(board.node("t").unwrap().text(), None);
}

#[test]
fn borrar_un_nodo_se_lleva_sus_flechas_y_nada_mas() {
    let mut board = board();
    board.remove(&ids(&["a"]));
    let remaining: Vec<&str> = board.edges.iter().map(|edge| edge.id.as_str()).collect();
    assert_eq!(remaining, ["bc"]);
    assert_eq!(board.nodes.len(), 2);
    Board::parse(&board.write()).unwrap();
}

#[test]
fn borrar_una_flecha_deja_sus_nodos() {
    let mut board = board();
    board.remove(&ids(&["ab"]));
    assert_eq!(board.edges.len(), 1);
    assert_eq!(board.nodes.len(), 3);
}

#[rstest]
#[case(Side::Right, 200, 0)]
#[case(Side::Left, -200, 0)]
#[case(Side::Bottom, 0, 150)]
#[case(Side::Top, 0, -150)]
fn una_caja_unida_nace_hacia_ese_lado(#[case] side: Side, #[case] x: i32, #[case] y: i32) {
    let mut board = Board::parse(
        r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":100,"height":50,"text":""}]}"#,
    )
    .unwrap();
    let id = board.add_linked_box("a", side).unwrap();
    let node = board.node(&id).unwrap();
    assert_eq!((node.x, node.y), (x, y));
    let edge = board.edges.last().unwrap();
    assert_eq!(
        (edge.from.node.as_str(), edge.to.node.as_str()),
        ("a", id.as_str())
    );
    assert_eq!(edge.to.tip, Tip::Arrow);
    Board::parse(&board.write()).unwrap();
}

#[test]
fn una_caja_unida_salta_lo_que_ya_esta_ahi() {
    let mut board = Board::parse(
        r#"{"nodes":[
        {"id":"a","type":"text","x":0,"y":0,"width":100,"height":50,"text":""},
        {"id":"b","type":"text","x":200,"y":0,"width":100,"height":50,"text":""}
    ]}"#,
    )
    .unwrap();
    let id = board.add_linked_box("a", Side::Right).unwrap();
    assert_eq!(board.node(&id).unwrap().x, 400);
}

#[test]
fn unir_con_algo_que_no_esta_no_hace_nada() {
    let mut board = board();
    assert_eq!(board.connect("a", "no"), None);
    assert_eq!(board.add_linked_box("no", Side::Top), None);
    assert_eq!(board.edges.len(), 2);
}

#[test]
fn un_id_nuevo_no_choca_con_nada() {
    let board = board();
    let (one, other) = (board.fresh_id(), board.fresh_id());
    assert_ne!(one, other);
    assert_eq!(one.len(), 16);
}

#[test]
fn crear_no_deja_una_forma_justo_sobre_otra() {
    let board = board();
    let frame = Frame {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    assert_eq!(board.unstacked(frame).x, 2 * GRID);
}

#[test]
fn un_trazo_tiene_la_caja_de_sus_puntos_y_vuelve_a_leerse_igual() {
    let mut board = Board::default();
    let points = [
        Point::new(10.5, 20.0),
        Point::new(40.0, 5.25),
        Point::new(-3.0, 60.0),
    ];
    let id = board.add_stroke(&points, 2.0).unwrap();
    let node = board.node(&id).unwrap();
    assert_eq!((node.x, node.y, node.width, node.height), (-3, 5, 43, 55));
    let NodeKind::Stroke { points: stored, .. } = &node.kind else {
        panic!("no es un trazo");
    };
    assert_eq!(stored[0].position(), Point::new(13.5, 15.0));
    let reread = Board::parse(&board.write()).unwrap();
    assert_eq!(reread, board);
}

#[test]
fn sin_puntos_no_hay_trazo() {
    assert_eq!(Board::default().add_stroke(&[], 2.0), None);
}

#[test]
fn una_tarjeta_nueva_lleva_la_ruta_de_la_nota() {
    let mut board = Board::default();
    let frame = Frame {
        x: 0,
        y: 0,
        width: 200,
        height: 80,
    };
    let id = board.add_card(frame, "carpeta/una nota.md");
    let reread = Board::parse(&board.write()).unwrap();
    assert!(matches!(
        &reread.node(&id).unwrap().kind,
        NodeKind::File { file, subpath: None } if file == "carpeta/una nota.md"
    ));
}

/// El lector admite cualquier `i32`: editar en esos extremos no se desborda.
#[test]
fn las_coordenadas_en_el_borde_de_un_i32_no_se_desbordan() {
    let extreme = format!(
        r#"{{"nodes":[{{"id":"a","type":"text","x":{max},"y":{max},"width":{max},"height":{max},"text":""}},{{"id":"b","type":"text","x":{min},"y":{min},"width":10,"height":10,"text":""}}]}}"#,
        max = i32::MAX,
        min = i32::MIN,
    );
    let mut board = Board::parse(&extreme).unwrap();
    board.move_nodes(&ids(&["a"]), 10, 10);
    board.move_nodes(&ids(&["b"]), -10, -10);
    assert_eq!(node_at(&board, "a"), (i32::MAX, i32::MAX));
    assert_eq!(node_at(&board, "b"), (i32::MIN, i32::MIN));
    let copied = board.copy(&ids(&["a", "b"]));
    assert_eq!(board.paste(&copied, Point::new(0.0, 0.0)).len(), 2);
    assert!(board.add_linked_box("a", Side::Right).is_some());
    assert!(board.add_linked_box("b", Side::Left).is_some());
    let frame = board.node("a").unwrap().frame();
    let dragged = frame.dragged(Corner::TopLeft, Point::new(f32::MAX, f32::MIN));
    assert!(dragged.width >= GRID);
    assert_eq!(board.unstacked(frame).x, i32::MAX);
    assert_eq!(snap(f32::MAX), i32::MAX);
    Board::parse(&board.write()).unwrap();
}

fn node_at(board: &Board, id: &str) -> (i32, i32) {
    let node = board.node(id).unwrap();
    (node.x, node.y)
}
