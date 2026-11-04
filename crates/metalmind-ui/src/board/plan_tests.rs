use metalmind_board::Board;
use pretty_assertions::assert_eq;

use super::*;
use crate::board::scene::SceneStyle;

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

const AREA: AreaSize = AreaSize {
    width: 1920.0,
    height: 1080.0,
};

fn scene(elements: usize) -> Scene {
    let board = Board::parse(&metalmind_perf::board(elements)).unwrap();
    Scene::build(&board, STYLE).unwrap()
}

fn looks(plan: &Plan<'_>, look: Look) -> usize {
    plan.elements.iter().filter(|(_, l)| *l == look).count()
}

#[test]
fn al_100_solo_pinta_lo_que_cae_en_la_ventana() {
    let scene = scene(2_000);
    let camera = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    let plan = plan(&scene, camera, AREA);
    let total = scene.nodes.len() + scene.edges.len();
    assert!(plan.elements.len() < total / 4, "{}", plan.elements.len());
    assert!(!plan.elements.is_empty());
    assert_eq!(looks(&plan, Look::Quad), 0);
    let shown = camera.visible(AREA);
    assert!(
        plan.elements
            .iter()
            .all(|(e, _)| e.bounds.intersects(&shown))
    );
}

/// Lo que obliga el tope de 128 MB de GPUI (09 §3.3): sin nivel de detalle,
/// 2 000 elementos de lejos serían 279 k triángulos y 20 fps.
#[test]
fn de_lejos_los_nodos_son_quads() {
    let scene = scene(2_000);
    let content = scene.content.unwrap();
    let camera = Camera::fit(content, AREA, 40.0);
    assert!(camera.zoom < DETAIL_ZOOM, "{}", camera.zoom);
    let plan = plan(&scene, camera, AREA);
    assert_eq!(
        looks(&plan, Look::Quad),
        scene.nodes.len() + scene.groups.len()
    );
    assert!(plan.triangles < MAX_TRIANGLES);
}

#[test]
fn nunca_pasa_del_tope_aunque_no_quepa_todo() {
    let scene = scene(40_000);
    let content = scene.content.unwrap();
    // Justo sobre el nivel de detalle: con mallas y con toda la pizarra.
    let camera = Camera {
        x: content.x,
        y: content.y,
        zoom: DETAIL_ZOOM,
    };
    let huge = AreaSize {
        width: content.w * DETAIL_ZOOM,
        height: content.h * DETAIL_ZOOM,
    };
    let plan = plan(&scene, camera, huge);
    assert!(plan.triangles <= MAX_TRIANGLES);
    // Los nodos siempre se ven: lo que no cabe como malla va como quad.
    let boxes = plan
        .elements
        .iter()
        .filter(|(element, look)| *look == Look::Quad || element.fill_role().is_some())
        .count();
    assert_eq!(boxes, scene.nodes.len() + scene.groups.len());
    assert!(looks(&plan, Look::Quad) > 0);
}

#[test]
fn los_grupos_van_abajo_y_los_nodos_sobre_las_flechas() {
    let scene = scene(100);
    let camera = Camera::fit(scene.content.unwrap(), AREA, 40.0);
    let plan = plan(&scene, camera, AREA);
    let first_edge = plan
        .elements
        .iter()
        .position(|(element, _)| element.fill_role().is_none());
    let last_group = plan.elements.iter().rposition(|(element, _)| {
        matches!(
            element.fill_role(),
            Some(crate::board::tint::Role::Group(_))
        )
    });
    if let (Some(group), Some(edge)) = (last_group, first_edge) {
        assert!(group < edge);
    }
    let last_edge = plan
        .elements
        .iter()
        .rposition(|(element, _)| element.fill_role().is_none());
    let first_node = plan.elements.iter().position(|(element, _)| {
        matches!(element.fill_role(), Some(crate::board::tint::Role::Fill(_)))
    });
    assert!(last_edge.unwrap() < first_node.unwrap());
}
