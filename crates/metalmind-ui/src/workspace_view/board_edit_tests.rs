//! Dibujar en una ventana de verdad: las teclas por el keymap y el ratón por
//! sus eventos, viendo lo que queda en el documento.

use gpui_kit::{
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    TestAppContext, point, px,
};
use metalmind_board::{Board, NodeKind, Shape};
use metalmind_keymap::BoardAction;
use metalmind_vim::Mode;
use pretty_assertions::assert_eq;

use super::board_tests::{BOARD, Window, open, open_with};
use crate::board::Tool;
use crate::document::Document;

const NODES: &str = r#"{
	"nodes":[
		{"id":"a","type":"text","x":0,"y":0,"width":200,"height":80,"text":"uno"},
		{"id":"b","type":"text","x":400,"y":0,"width":200,"height":80,"text":"dos"},
		{"id":"c","type":"text","x":0,"y":300,"width":200,"height":80,"text":"tres"}
	],
	"edges":[
		{"id":"ab","fromNode":"a","toNode":"b"}
	]
}
"#;

fn on_board(cx: &mut TestAppContext) -> Window<'_> {
    let mut window = open_with(cx, "idea.canvas", NODES);
    window.keys("shift-0");
    window
}

impl Window<'_> {
    fn parsed(&mut self) -> Board {
        Board::parse(&self.text()).unwrap()
    }

    fn with_document<T>(&mut self, read: impl FnOnce(&Document) -> T) -> T {
        self.view.read_with(self.cx, |view, cx| {
            read(view.workspace.read(cx).focused_document().unwrap())
        })
    }

    fn mode(&mut self) -> Mode {
        self.view.read_with(self.cx, |view, cx| {
            let workspace = view.workspace.read(cx);
            workspace.view(workspace.focused()).unwrap().mode
        })
    }

    fn selected(&mut self) -> Vec<String> {
        let board = self.board();
        board.read_with(self.cx, |board, _| board.all_selected().to_vec())
    }

    fn tool(&mut self) -> Tool {
        let board = self.board();
        board.read_with(self.cx, |board, _| board.tool())
    }

    fn at_board(&mut self, x: f32, y: f32) -> Point<Pixels> {
        let board = self.board();
        board.read_with(self.cx, |board, _| {
            board
                .window_point(metalmind_board::Point::new(x, y))
                .unwrap()
        })
    }

    fn center(&mut self, id: &str) -> Point<Pixels> {
        let board = self.board();
        board.read_with(self.cx, |board, _| board.window_position(id).unwrap())
    }

    fn down(&mut self, position: Point<Pixels>, modifiers: Modifiers, click_count: usize) {
        self.cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count,
            first_mouse: false,
        });
    }

    fn drag_to(&mut self, position: Point<Pixels>) {
        self.cx.simulate_event(MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        });
    }

    fn up(&mut self, position: Point<Pixels>) {
        self.cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::none(),
            click_count: 1,
        });
        self.cx.run_until_parked();
    }

    fn click(&mut self, position: Point<Pixels>) {
        self.down(position, Modifiers::none(), 1);
        self.up(position);
    }

    fn drag(&mut self, from: Point<Pixels>, to: Point<Pixels>) {
        self.down(from, Modifiers::none(), 1);
        // Dos pasos: un solo salto no siempre pasa el umbral antes de soltar.
        self.drag_to(from + (to - from) * 0.5);
        self.drag_to(to);
        self.up(to);
    }

    fn undo_to_start(&mut self, start: &str) {
        for _ in 0..50 {
            if self.text().as_str() == start {
                return;
            }
            self.keys("u");
        }
        assert_eq!(self.text(), start);
    }
}

fn node_xy(board: &Board, id: &str) -> (i32, i32) {
    let node = board.node(id).unwrap();
    (node.x, node.y)
}

fn newest_node(board: &Board) -> &metalmind_board::Node {
    board.nodes.last().unwrap()
}

// --- Elegir y mover con el teclado

#[gpui_kit::test]
fn con_algo_elegido_h_j_k_l_lo_mueven_y_no_la_vista(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    let board = window.board();
    let camera = board.read_with(window.cx, |board, _| board.camera().unwrap());
    window.keys("l j");
    assert_eq!(node_xy(&window.parsed(), "a"), (10, 10));
    window.keys("h h k");
    assert_eq!(node_xy(&window.parsed(), "a"), (-10, 0));
    let after = board.read_with(window.cx, |board, _| board.camera().unwrap());
    assert_eq!((camera.x, camera.y), (after.x, after.y));
}

#[gpui_kit::test]
fn mover_toca_solo_la_linea_de_lo_movido(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    let before = window.text();
    window.keys("l");
    let after = window.text();
    let changed: Vec<_> = before
        .lines()
        .zip(after.lines())
        .filter(|(old, new)| old != new)
        .collect();
    assert_eq!(changed.len(), 1);
    assert!(changed[0].1.contains(r#""id":"a""#));
    assert_eq!(before.lines().count(), after.lines().count());
}

#[gpui_kit::test]
fn escape_suelta_lo_elegido(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    window.keys("escape");
    assert_eq!(window.selected(), Vec::<String>::new());
}

// --- Crear

fn creates_with_enter(cx: &mut TestAppContext, key: &str, shape: Shape) {
    let mut window = on_board(cx);
    let nodes = window.parsed().nodes.len();
    window.keys(key);
    assert_eq!(window.tool(), Tool::Shape(shape));
    window.keys("enter");
    assert_eq!(window.tool(), Tool::Select);
    let parsed = window.parsed();
    assert_eq!(parsed.nodes.len(), nodes + 1);
    let node = newest_node(&parsed);
    assert!(matches!(node.kind, NodeKind::Text { shape: found, .. } if found == shape));
    assert_eq!(node.x % 10, 0);
    assert_eq!(window.selected(), std::slice::from_ref(&node.id));
}

#[gpui_kit::test]
fn r_y_enter_crean_una_caja_en_el_centro_y_vuelven_a_elegir(cx: &mut TestAppContext) {
    creates_with_enter(cx, "r", Shape::Rectangle);
}

#[gpui_kit::test]
fn o_y_enter_crean_una_elipse(cx: &mut TestAppContext) {
    creates_with_enter(cx, "o", Shape::Ellipse);
}

#[gpui_kit::test]
fn d_y_enter_crean_un_rombo(cx: &mut TestAppContext) {
    creates_with_enter(cx, "d", Shape::Diamond);
}

#[gpui_kit::test]
fn con_una_forma_un_clic_la_crea_ahi_y_arrastrar_le_da_su_tamano(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    window.keys("o");
    let at = window.at_board(800.0, 500.0);
    window.click(at);
    let parsed = window.parsed();
    let node = newest_node(&parsed);
    assert_eq!((node.x, node.width), (700, 200));
    assert_eq!(window.tool(), Tool::Select);
    window.keys("r");
    let (from, to) = (window.at_board(303.0, 104.0), window.at_board(548.0, 212.0));
    window.drag(from, to);
    let parsed = window.parsed();
    let node = newest_node(&parsed);
    assert_eq!(
        (node.x, node.y, node.width, node.height),
        (300, 100, 250, 110)
    );
}

#[gpui_kit::test]
fn una_forma_nueva_no_cae_justo_sobre_otra(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    window.keys("r enter r enter");
    let parsed = window.parsed();
    let (one, other) = (&parsed.nodes[3], &parsed.nodes[4]);
    assert_ne!((one.x, one.y), (other.x, other.y));
}

#[gpui_kit::test]
fn t_crea_un_texto_suelto_y_lo_abre_para_escribir(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    window.keys("t enter");
    assert_eq!(window.mode(), Mode::Insert);
    window.cx.simulate_input("una idea");
    window.keys("escape");
    window.keys("escape");
    let parsed = window.parsed();
    let node = newest_node(&parsed);
    assert_eq!(node.text(), Some("una idea"));
    assert!(matches!(
        node.kind,
        NodeKind::Text {
            shape: Shape::None,
            ..
        }
    ));
    window.keys("u");
    assert_eq!(window.text(), start);
}

#[gpui_kit::test]
fn un_texto_suelto_sin_nada_escrito_no_deja_rastro(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    window.keys("t enter escape escape");
    assert_eq!(window.text(), start);
    assert!(!window.modified());
}

// --- Flechas con el teclado

#[gpui_kit::test]
fn a_y_una_direccion_crean_una_caja_unida_hacia_ese_lado(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("c");
    window.click(at);
    window.keys("a l");
    let parsed = window.parsed();
    let new = newest_node(&parsed);
    assert!(new.x > 200);
    assert_eq!(new.y, 300);
    let edge = parsed.edges.last().unwrap();
    assert_eq!(
        (edge.from.node.as_str(), edge.to.node.as_str()),
        ("c", new.id.as_str())
    );
    assert_eq!(window.selected(), std::slice::from_ref(&new.id));
    assert_eq!(window.tool(), Tool::Select);
}

#[gpui_kit::test]
fn sin_nada_elegido_a_y_una_direccion_mueven_la_vista(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let nodes = window.parsed().nodes.len();
    window.keys("a l");
    assert_eq!(window.parsed().nodes.len(), nodes);
}

#[gpui_kit::test]
fn f_y_la_letra_de_un_nodo_lo_unen(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("c");
    window.click(at);
    window.keys("a f");
    let board = window.board();
    let labels = board.read_with(window.cx, |board, _| board.hint_labels());
    assert_eq!(labels.len(), 2);
    let letter = labels.iter().find(|(id, _)| id == "b").unwrap().1.clone();
    window.keys(&letter);
    let parsed = window.parsed();
    let edge = parsed.edges.last().unwrap();
    assert_eq!((edge.from.node.as_str(), edge.to.node.as_str()), ("c", "b"));
    assert!(board.read_with(window.cx, |board, _| !board.hints_open()));
}

#[gpui_kit::test]
fn escape_o_una_letra_de_nadie_dejan_las_letras_sin_unir(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("c");
    window.click(at);
    let before = window.text();
    window.keys("f");
    window.keys("escape");
    window.keys("f");
    window.keys("z");
    assert_eq!(window.text(), before);
    let board = window.board();
    assert!(board.read_with(window.cx, |board, _| !board.hints_open()));
    assert_eq!(window.selected(), ["c".to_owned()]);
}

// --- Texto

#[gpui_kit::test]
fn i_abre_la_caja_en_insert_y_esc_en_normal_vuelve_con_un_solo_paso(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    let start = window.text();
    window.keys("i");
    assert_eq!(window.mode(), Mode::Insert);
    window.cx.simulate_input(" más");
    assert_eq!(window.parsed().node("a").unwrap().text(), Some("uno más"));
    window.keys("escape");
    assert!(window.with_document(|d| d.open_board_text().is_some()));
    window.keys("0 x");
    assert_eq!(window.parsed().node("a").unwrap().text(), Some("no más"));
    window.keys("u");
    assert_eq!(window.parsed().node("a").unwrap().text(), Some("uno más"));
    window.keys("escape");
    assert!(window.with_document(|d| d.open_board_text().is_none()));
    assert_eq!(window.mode(), Mode::Normal);
    window.keys("u");
    assert_eq!(window.text(), start);
    window.keys("ctrl-r");
    assert_eq!(window.parsed().node("a").unwrap().text(), Some("uno más"));
}

#[gpui_kit::test]
fn una_caja_crece_al_salir_si_su_texto_no_cabe_y_es_el_mismo_paso(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    let start = window.text();
    window.keys("i");
    for _ in 0..9 {
        window.keys("enter");
    }
    window.cx.simulate_input("fin");
    window.keys("escape escape");
    let parsed = window.parsed();
    let node = parsed.node("a").unwrap();
    assert!(node.height > 80, "{}", node.height);
    assert_eq!(node.height % 10, 0);
    window.keys("u");
    assert_eq!(window.text(), start);
}

#[gpui_kit::test]
fn una_caja_a_la_que_le_cabe_el_texto_no_cambia_de_tamano(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    window.keys("i");
    window.cx.simulate_input("!");
    window.keys("escape escape");
    assert_eq!(window.parsed().node("a").unwrap().height, 80);
}

#[gpui_kit::test]
fn enter_y_doble_clic_tambien_abren_la_caja(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    window.keys("enter");
    assert_eq!(window.mode(), Mode::Insert);
    window.keys("escape escape");
    window.click(at);
    window.down(at, Modifiers::none(), 2);
    window.up(at);
    assert_eq!(window.mode(), Mode::Insert);
}

#[gpui_kit::test]
fn un_clic_fuera_de_la_caja_la_cierra_sin_perder_lo_escrito(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    window.keys("i");
    window.cx.simulate_input(" escrito");
    let elsewhere = window.center("b");
    window.click(elsewhere);
    assert!(window.with_document(|d| d.open_board_text().is_none()));
    assert_eq!(window.mode(), Mode::Normal);
    assert_eq!(
        window.parsed().node("a").unwrap().text(),
        Some("uno escrito")
    );
    assert_eq!(window.selected(), ["b".to_owned()]);
}

#[gpui_kit::test]
fn un_clic_dentro_de_la_caja_abierta_la_deja_abierta(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    window.keys("i");
    window.click(at);
    assert!(window.with_document(|d| d.open_board_text().is_some()));
}

#[gpui_kit::test]
fn las_letras_en_una_caja_son_del_motor_y_no_de_la_pizarra(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    window.keys("i escape");
    let nodes = window.parsed().nodes.len();
    window.keys("r o d");
    assert_eq!(window.tool(), Tool::Select);
    assert_eq!(window.parsed().nodes.len(), nodes);
}

#[gpui_kit::test]
fn una_tarjeta_no_se_escribe_y_lo_dice(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    let at = window.center("tarjeta");
    window.click(at);
    window.keys("i");
    assert_eq!(window.mode(), Mode::Normal);
    assert_eq!(window.on_disk("idea.canvas"), BOARD);
}

// --- Borrar, copiar y pegar, deshacer

#[gpui_kit::test]
fn x_borra_el_nodo_y_sus_flechas_y_deshacer_devuelve_los_bytes(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let at = window.center("a");
    window.click(at);
    window.keys("x");
    let parsed = window.parsed();
    assert!(parsed.node("a").is_none());
    assert_eq!(parsed.edges.len(), 0);
    assert_eq!(window.selected(), Vec::<String>::new());
    let deleted = window.text();
    window.keys("u");
    assert_eq!(window.text(), start);
    window.keys("ctrl-r");
    assert_eq!(window.text(), deleted);
}

#[gpui_kit::test]
fn delete_borra_una_flecha_elegida_con_un_clic_y_deja_sus_nodos(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let parsed = window.parsed();
    let (_, geometry) = parsed.edge_geometries().next().unwrap();
    let middle = geometry.label_at;
    let at = window.at_board(middle.x, middle.y);
    window.click(at);
    assert_eq!(window.selected(), ["ab".to_owned()]);
    window.keys("delete");
    let parsed = window.parsed();
    assert_eq!(parsed.edges.len(), 0);
    assert_eq!(parsed.nodes.len(), 3);
}

#[gpui_kit::test]
fn y_y_p_mayuscula_pegan_con_ids_nuevos_y_corridos(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("a");
    window.click(at);
    window.keys("y");
    let copied = window.cx.read_from_clipboard().unwrap().text().unwrap();
    assert!(copied.starts_with(r#"{"id":"a","type":"text""#));
    window.keys("shift-p");
    let parsed = window.parsed();
    assert_eq!(parsed.nodes.len(), 4);
    let pasted = newest_node(&parsed);
    assert_ne!(pasted.id, "a");
    assert_eq!((pasted.x, pasted.y), (20, 20));
    assert_eq!(pasted.text(), Some("uno"));
    assert_eq!(window.selected(), std::slice::from_ref(&pasted.id));
}

#[gpui_kit::test]
fn todo_se_deshace_hasta_los_bytes_del_principio(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let at = window.center("a");
    window.click(at);
    window.keys("l j");
    window.keys("i");
    window.cx.simulate_input("!");
    window.keys("escape escape");
    window.keys("a l");
    window.keys("r enter");
    window.keys("x");
    let end = window.text();
    window.undo_to_start(&start);
    assert!(!window.modified());
    for _ in 0..6 {
        window.keys("ctrl-r");
    }
    assert_eq!(window.text(), end);
}

// --- Ratón

#[gpui_kit::test]
fn arrastrar_un_nodo_lo_mueve_en_la_rejilla_y_es_un_solo_paso(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let from = window.center("c");
    let to = from + point(px(133.), px(47.));
    window.drag(from, to);
    assert_eq!(node_xy(&window.parsed(), "c"), (130, 350));
    assert_eq!(window.selected(), ["c".to_owned()]);
    window.keys("u");
    assert_eq!(window.text(), start);
}

#[gpui_kit::test]
fn arrastrando_el_archivo_no_cambia_hasta_soltar(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let from = window.center("c");
    window.down(from, Modifiers::none(), 1);
    window.drag_to(from + point(px(100.), px(0.)));
    window.drag_to(from + point(px(200.), px(0.)));
    window.cx.run_until_parked();
    assert_eq!(window.text(), start);
    let board = window.board();
    let seen = board.read_with(window.cx, |board, _| {
        node_xy(board.board_for_tests().unwrap(), "c")
    });
    assert_eq!(seen, (200, 300));
    window.up(from + point(px(200.), px(0.)));
    assert_eq!(node_xy(&window.parsed(), "c"), (200, 300));
}

#[gpui_kit::test]
fn arrastrar_alinea_con_un_vecino_cercano_antes_que_con_la_rejilla(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    // `c` mide lo mismo que `b`: 4 unidades de más quedan a menos de 6 píxeles.
    let from = window.center("c");
    window.drag(from, from + point(px(404.), px(-200.)));
    assert_eq!(node_xy(&window.parsed(), "c").0, 400);
}

#[gpui_kit::test]
fn escape_arrastrando_devuelve_el_nodo_sin_tocar_el_archivo(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let from = window.center("c");
    window.down(from, Modifiers::none(), 1);
    window.drag_to(from + point(px(200.), px(0.)));
    window.drag_to(from + point(px(300.), px(0.)));
    window.dispatch(BoardAction::Cancel);
    window.up(from + point(px(300.), px(0.)));
    assert_eq!(window.text(), start);
    let board = window.board();
    let seen = board.read_with(window.cx, |board, _| {
        node_xy(board.board_for_tests().unwrap(), "c")
    });
    assert_eq!(seen, (0, 300));
}

#[gpui_kit::test]
fn shift_clic_elige_varios_y_arrastrar_mueve_todos(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let (a, c) = (window.center("a"), window.center("c"));
    window.click(a);
    window.down(c, Modifiers::shift(), 1);
    window.up(c);
    assert_eq!(window.selected(), ["a".to_owned(), "c".to_owned()]);
    window.drag(c, c + point(px(50.), px(0.)));
    let parsed = window.parsed();
    assert_eq!(node_xy(&parsed, "a"), (50, 0));
    assert_eq!(node_xy(&parsed, "c"), (50, 300));
    window.keys("j");
    let parsed = window.parsed();
    assert_eq!(node_xy(&parsed, "a"), (50, 10));
    assert_eq!(node_xy(&parsed, "c"), (50, 310));
    window.keys("u u");
    assert_eq!(node_xy(&window.parsed(), "a"), (0, 0));
}

#[gpui_kit::test]
fn un_clic_sin_arrastrar_sobre_uno_de_varios_deja_solo_ese(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let (a, c) = (window.center("a"), window.center("c"));
    window.click(a);
    window.down(c, Modifiers::shift(), 1);
    window.up(c);
    window.click(a);
    assert_eq!(window.selected(), ["a".to_owned()]);
}

#[gpui_kit::test]
fn shift_y_arrastrar_en_el_vacio_selecciona_lo_que_cabe_y_no_mueve_la_vista(
    cx: &mut TestAppContext,
) {
    let mut window = on_board(cx);
    let board = window.board();
    let camera = board.read_with(window.cx, |board, _| board.camera().unwrap());
    let from = window.at_board(-50.0, -50.0);
    let to = window.at_board(250.0, 450.0);
    window.down(from, Modifiers::shift(), 1);
    window.drag_to(to);
    window.up(to);
    assert_eq!(window.selected(), ["a".to_owned(), "c".to_owned()]);
    let after = board.read_with(window.cx, |board, _| board.camera().unwrap());
    assert_eq!((camera.x, camera.y), (after.x, after.y));
}

#[gpui_kit::test]
fn arrastrar_desde_el_borde_a_otro_nodo_los_une(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let (from, to) = (window.at_board(200.0, 340.0), window.center("b"));
    window.drag(from, to);
    let parsed = window.parsed();
    let edge = parsed.edges.last().unwrap();
    assert_eq!((edge.from.node.as_str(), edge.to.node.as_str()), ("c", "b"));
    assert_eq!(parsed.nodes.len(), 3);
    assert_eq!(node_xy(&parsed, "c"), (0, 300));
}

#[gpui_kit::test]
fn soltar_una_flecha_en_el_vacio_crea_una_caja_unida(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let (from, to) = (window.at_board(200.0, 340.0), window.at_board(700.0, 600.0));
    window.drag(from, to);
    let parsed = window.parsed();
    assert_eq!(parsed.nodes.len(), 4);
    let new = newest_node(&parsed);
    assert_eq!((new.x, new.width), (600, 200));
    assert_eq!(window.selected(), std::slice::from_ref(&new.id));
    window.keys("u");
    assert_eq!(window.text(), start);
}

#[gpui_kit::test]
fn soltar_una_flecha_sobre_su_propio_nodo_no_hace_nada(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let (from, to) = (window.at_board(200.0, 340.0), window.center("c"));
    window.drag(from, to);
    assert_eq!(window.text(), start);
}

#[gpui_kit::test]
fn un_clic_en_el_borde_elige_el_nodo_sin_crear_nada(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let start = window.text();
    let at = window.at_board(200.0, 340.0);
    window.click(at);
    assert_eq!(window.selected(), ["c".to_owned()]);
    assert_eq!(window.text(), start);
}

#[gpui_kit::test]
fn con_la_herramienta_de_flecha_se_arrastra_desde_cualquier_parte_del_nodo(
    cx: &mut TestAppContext,
) {
    let mut window = on_board(cx);
    window.keys("a");
    let (from, to) = (window.center("c"), window.center("b"));
    window.drag(from, to);
    let parsed = window.parsed();
    let edge = parsed.edges.last().unwrap();
    assert_eq!((edge.from.node.as_str(), edge.to.node.as_str()), ("c", "b"));
    assert_eq!(window.tool(), Tool::Select);
}

#[gpui_kit::test]
fn una_esquina_de_lo_elegido_cambia_su_tamano_en_la_rejilla(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("c");
    window.click(at);
    let (from, to) = (window.at_board(200.0, 380.0), window.at_board(304.0, 417.0));
    window.drag(from, to);
    let parsed = window.parsed();
    let node = parsed.node("c").unwrap();
    assert_eq!(
        (node.x, node.y, node.width, node.height),
        (0, 300, 300, 120)
    );
    window.keys("u");
    assert_eq!(window.parsed().node("c").unwrap().width, 200);
}

#[gpui_kit::test]
fn ctrl_h_j_k_l_cambian_el_tamano_de_lo_elegido(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("c");
    window.click(at);
    window.keys("ctrl-l ctrl-j");
    let parsed = window.parsed();
    let node = parsed.node("c").unwrap();
    assert_eq!((node.width, node.height), (210, 90));
    window.keys("ctrl-h ctrl-k");
    let parsed = window.parsed();
    let node = parsed.node("c").unwrap();
    assert_eq!((node.width, node.height), (200, 80));
}

#[gpui_kit::test]
fn la_camara_sigue_donde_estaba_al_volver_a_la_pizarra(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    window.keys("+ l j");
    let board = window.board();
    let before = board.read_with(window.cx, |board, _| board.camera().unwrap());
    window.keys(":");
    window.cx.simulate_input("e a.md");
    window.keys("enter");
    assert_eq!(window.file(), "a.md");
    window.keys(":");
    let board_file = window.root.join("idea.canvas");
    window
        .cx
        .simulate_input(&format!("e {}", board_file.display()));
    window.keys("enter");
    let board = window.board();
    let after = board.read_with(window.cx, |board, _| board.camera().unwrap());
    assert_eq!(
        (before.x, before.y, before.zoom),
        (after.x, after.y, after.zoom)
    );
}

// --- Menú y barra

#[gpui_kit::test]
fn el_menu_contextual_borra_lo_que_hay_bajo_el_raton(cx: &mut TestAppContext) {
    let mut window = on_board(cx);
    let at = window.center("c");
    window.cx.simulate_event(MouseDownEvent {
        button: MouseButton::Right,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    window.cx.run_until_parked();
    assert_eq!(window.selected(), ["c".to_owned()]);
    let row = window.cx.debug_bounds("Borrar").unwrap().center();
    window.click(row);
    assert!(window.parsed().node("c").is_none());
    assert!(window.cx.debug_bounds("Borrar").is_none());
}

#[gpui_kit::test]
fn la_paleta_lista_las_acciones_de_la_pizarra_con_su_tecla(cx: &mut TestAppContext) {
    let window = on_board(cx);
    let keymap = window
        .cx
        .update(|_, cx| crate::actions::AppKeymap::get(cx).clone());
    for action in BoardAction::ALL {
        let view = match action {
            BoardAction::EndTextEdit => metalmind_keymap::View::BoardTextIdle,
            _ => metalmind_keymap::View::Board,
        };
        let keys = keymap.keys_for((*action).into(), |shown| shown == view);
        assert!(keys.is_some(), "{} sin tecla", action.name());
    }
}
