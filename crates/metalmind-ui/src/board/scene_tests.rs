use metalmind_board::Board;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;
use crate::board::tint::{Role, Tint};

const STYLE: SceneStyle = SceneStyle {
    line_width: 2.0,
    radius: 8.0,
    padding: 12.0,
    arrow_size: 12.0,
    card_accent: 4.0,
    text: 16.0,
    text_small: 13.0,
    line_height: 1.3,
    edge_label_width: 160.0,
};

const EXAMPLE: &str = include_str!("../../../metalmind-board/tests/fixtures/arquitectura.canvas");

fn scene(text: &str) -> Scene {
    Scene::build(&Board::parse(text).unwrap(), STYLE).unwrap()
}

fn node(fields: &str) -> String {
    format!(r#"{{"nodes":[{{"id":"a","x":100,"y":50,"width":200,"height":100,{fields}}}]}}"#)
}

#[test]
fn cada_elemento_del_ejemplo_tiene_su_escena() {
    let board = Board::parse(EXAMPLE).unwrap();
    let scene = Scene::build(&board, STYLE).unwrap();
    assert_eq!(scene.groups.len() + scene.nodes.len(), board.nodes.len());
    assert_eq!(scene.edges.len(), board.edges.len());
    let content = scene.content.unwrap();
    for element in scene.nodes.iter().chain(&scene.edges) {
        let b = element.bounds;
        assert!(content.x <= b.x && content.y <= b.y);
        assert!(content.x + content.w >= b.x + b.w - 0.01);
    }
}

#[test]
fn una_pizarra_vacia_no_tiene_contenido() {
    let scene = scene("");
    assert!(scene.content.is_none());
    assert!(scene.nodes.is_empty());
}

#[rstest]
#[case(r#""type":"text","text":"hola""#, LabelKind::Body, "hola")]
#[case(r#""type":"text","text":"**fuerte** x""#, LabelKind::Body, "fuerte x")]
#[case(r##""type":"text","text":"# Título""##, LabelKind::Heading, "Título")]
#[case(
    r#""type":"text","shape":"none","text":"suelto""#,
    LabelKind::Free,
    "suelto"
)]
#[case(r#""type":"file","file":"a/nota.md""#, LabelKind::CardTitle, "nota")]
#[case(
    r##""type":"file","file":"a/nota.md","subpath":"#Riesgos""##,
    LabelKind::CardTitle,
    "nota#Riesgos"
)]
#[case(
    r#""type":"file","file":"datos.csv""#,
    LabelKind::CardTitle,
    "datos.csv"
)]
#[case(
    r#""type":"link","url":"https://x.y""#,
    LabelKind::CardPath,
    "https://x.y"
)]
fn el_texto_de_cada_nodo(#[case] fields: &str, #[case] kind: LabelKind, #[case] text: &str) {
    let scene = scene(&node(fields));
    let label = &scene.nodes[0].labels[0];
    assert_eq!(label.kind, kind);
    assert_eq!(label.text.as_ref(), text);
}

#[test]
fn una_tarjeta_de_nota_muestra_su_ruta_bajo_el_titulo() {
    let scene = scene(&node(r#""type":"file","file":"a/nota.md""#));
    let [title, path] = &scene.nodes[0].labels[..] else {
        panic!("título y ruta");
    };
    assert_eq!(path.text.as_ref(), "a/nota.md");
    assert_eq!(path.kind, LabelKind::CardPath);
    assert!(path.area.y >= title.area.y + title.area.h);
    assert!(
        scene.nodes[0]
            .pieces
            .iter()
            .any(|p| p.role == Role::CardAccent)
    );
}

#[test]
fn un_texto_suelto_no_tiene_caja() {
    let scene = scene(&node(r#""type":"text","shape":"none","text":"x""#));
    assert!(scene.nodes[0].pieces.is_empty());
    assert_eq!(scene.nodes[0].triangles(), 0);
}

#[test]
fn un_grupo_va_al_fondo_con_su_nombre_arriba() {
    let scene = scene(&node(r#""type":"group","label":"Todo""#));
    assert!(scene.nodes.is_empty());
    let group = &scene.groups[0];
    assert_eq!(group.labels[0].kind, LabelKind::Group);
    assert!(group.labels[0].area.y < group.outline.y);
    assert!(group.bounds.y < group.outline.y);
}

#[test]
fn el_color_del_archivo_llega_a_las_piezas() {
    let scene = scene(&node(r#""type":"text","text":"x","color":"4""#));
    let line = scene.nodes[0].line_role().unwrap();
    assert_eq!(line, Role::Line(Tint::parse(Some("4"))));
}

#[rstest]
#[case(r#""fromEnd":"none","toEnd":"arrow""#, 1)]
#[case(r#""fromEnd":"arrow","toEnd":"arrow""#, 2)]
#[case(r#""fromEnd":"none","toEnd":"none""#, 0)]
fn una_flecha_tiene_una_malla_por_punta(#[case] ends: &str, #[case] tips: usize) {
    let text = format!(
        r#"{{"nodes":[
        {{"id":"a","type":"text","text":"","x":0,"y":0,"width":100,"height":100}},
        {{"id":"b","type":"text","text":"","x":400,"y":0,"width":100,"height":100}}],
        "edges":[{{"id":"e","fromNode":"a","toNode":"b",{ends},"label":"lee"}}]}}"#
    );
    let scene = scene(&text);
    let edge = &scene.edges[0];
    assert_eq!(edge.pieces.len(), 1 + tips);
    assert_eq!(edge.labels[0].kind, LabelKind::Edge);
}

/// Lo que se ve de cada elemento, para comparar dos escenas.
fn fingerprint(scene: &Scene) -> Vec<String> {
    let of = |element: &ElementScene| {
        format!(
            "{:?} {:?} {} {:?}",
            element.outline,
            element.bounds,
            element.triangles(),
            element
                .labels
                .iter()
                .map(|l| (l.text.clone(), l.area))
                .collect::<Vec<_>>()
        )
    };
    scene
        .groups
        .iter()
        .chain(&scene.nodes)
        .chain(&scene.edges)
        .map(of)
        .collect()
}

#[test]
fn reconstruir_lo_movido_da_lo_mismo_que_construir_todo() {
    let mut board = Board::parse(EXAMPLE).unwrap();
    let mut scene = Scene::build(&board, STYLE).unwrap();
    let id = board
        .nodes
        .iter()
        .find(|node| !board.edges.iter().all(|e| e.from.node != node.id))
        .unwrap()
        .id
        .clone();
    let node = board.nodes.iter_mut().find(|node| node.id == id).unwrap();
    node.x += 130;
    node.y -= 50;
    scene
        .update_nodes(&board, &HashSet::from([id.as_str()]), STYLE)
        .unwrap();
    let whole = Scene::build(&board, STYLE).unwrap();
    assert_eq!(fingerprint(&scene), fingerprint(&whole));
    assert_eq!(scene.content, whole.content);
}

#[rstest]
#[case("mover", |board: &mut Board| board.nodes[1].x += 10)]
#[case("escribir", |board: &mut Board| { let id = board.nodes[2].id.clone(); board.set_text(&id, "otro"); })]
#[case("etiqueta", |board: &mut Board| { let id = board.edges[0].id.clone(); board.set_text(&id, "lleva a"); })]
#[case("borrar", |board: &mut Board| { let id = board.nodes[1].id.clone(); board.remove(&[id]); })]
#[case("crear", |board: &mut Board| { board.add_text(metalmind_board::Frame { x: 0, y: 0, width: 10, height: 10 }, metalmind_board::Shape::None, "x"); })]
fn refrescar_da_lo_mismo_que_construir_todo(#[case] name: &str, #[case] change: fn(&mut Board)) {
    let old = Board::parse(EXAMPLE).unwrap();
    let mut new = old.clone();
    change(&mut new);
    let mut scene = Scene::build(&old, STYLE).unwrap();
    scene.refresh(&old, &new, STYLE).unwrap();
    let whole = Scene::build(&new, STYLE).unwrap();
    assert_eq!(fingerprint(&scene), fingerprint(&whole), "{name}");
}

#[test]
fn abrir_una_etiqueta_la_deja_sin_texto_y_dice_donde_va() {
    let board = Board::parse(EXAMPLE).unwrap();
    let mut scene = Scene::build(&board, STYLE).unwrap();
    let text_node = board
        .nodes
        .iter()
        .find(|node| node.text().is_some())
        .unwrap();
    let open = scene.open_label(&board, &text_node.id, STYLE).unwrap();
    assert!(open.area.w > 0.0);
    let unlabelled = board
        .edges
        .iter()
        .find(|edge| edge.label.is_none())
        .unwrap();
    let open_edge = scene.open_label(&board, &unlabelled.id, STYLE).unwrap();
    assert_eq!(open_edge.kind, LabelKind::Edge);
    assert!(scene.open_label(&board, "no-existe", STYLE).is_none());
}
