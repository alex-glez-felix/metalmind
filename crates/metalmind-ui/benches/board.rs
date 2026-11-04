//! La pizarra sin ventana: leerla, teselarla y decidir qué se pinta (09 §3.2).
//! `cargo bench -p metalmind-ui --bench board`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::collections::HashSet;

use divan::{Bencher, black_box};
use metalmind_board::Board;
use metalmind_ui::board::{AreaSize, Camera, Scene, SceneStyle, plan};

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

fn main() {
    divan::main();
}

#[divan::bench(args = [500, 2_000])]
fn leer(bencher: Bencher, elements: usize) {
    let text = metalmind_perf::board(elements);
    bencher.bench(|| Board::parse(black_box(&text)).unwrap());
}

#[divan::bench(args = [500, 2_000])]
fn construir_la_escena(bencher: Bencher, elements: usize) {
    let board = Board::parse(&metalmind_perf::board(elements)).unwrap();
    bencher.bench(|| Scene::build(black_box(&board), STYLE).unwrap());
}

/// Un frame de arrastrar una caja con sus flechas: mover el nodo en la pizarra
/// leída y reconstruir solo eso (09 §3.1).
#[divan::bench(args = [500, 2_000])]
fn arrastrar_una_caja(bencher: Bencher, elements: usize) {
    let mut board = Board::parse(&metalmind_perf::board(elements)).unwrap();
    let mut scene = Scene::build(&board, STYLE).unwrap();
    let id = board.edges[0].from.node.clone();
    let mut x = 0;
    bencher.bench_local(|| {
        x = (x + 10) % 1000;
        if let Some(node) = board.nodes.iter_mut().find(|node| node.id == id) {
            node.x = x;
        }
        let changed = HashSet::from([id.as_str()]);
        scene
            .update_nodes(black_box(&board), &changed, STYLE)
            .unwrap();
    });
}

/// Soltar la caja: el archivo vuelve a la pizarra y solo cambia un elemento.
#[divan::bench(args = [500, 2_000])]
fn soltar_una_caja(bencher: Bencher, elements: usize) {
    let old = Board::parse(&metalmind_perf::board(elements)).unwrap();
    let mut new = old.clone();
    new.nodes[1].x += 10;
    let mut scene = Scene::build(&old, STYLE).unwrap();
    bencher.bench_local(|| {
        scene
            .refresh(black_box(&old), black_box(&new), STYLE)
            .unwrap();
    });
}

/// Editar el archivo como un paso del historial: leerlo, cambiarlo, escribirlo
/// y sacar las líneas que cambiaron (lo que cuesta cada `h j k l` y cada tecla
/// en una caja).
#[divan::bench(args = [500, 2_000])]
fn las_lineas_de_una_edicion(bencher: Bencher, elements: usize) {
    let text = metalmind_perf::board(elements);
    let text = text.strip_suffix('\n').unwrap_or(&text).to_owned();
    bencher.bench(|| {
        let mut board = Board::parse(black_box(&text)).unwrap();
        board.nodes[1].x += 10;
        let written = board.write();
        metalmind_board::line_edits(&text, written.strip_suffix('\n').unwrap_or(&written)).len()
    });
}

#[divan::bench(args = [500, 2_000])]
fn plan_al_100(bencher: Bencher, elements: usize) {
    let board = Board::parse(&metalmind_perf::board(elements)).unwrap();
    let scene = Scene::build(&board, STYLE).unwrap();
    let camera = Camera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    };
    bencher.bench(|| plan(black_box(&scene), camera, AREA).triangles);
}

#[divan::bench(args = [500, 2_000])]
fn plan_de_lejos(bencher: Bencher, elements: usize) {
    let board = Board::parse(&metalmind_perf::board(elements)).unwrap();
    let scene = Scene::build(&board, STYLE).unwrap();
    let camera = Camera::fit(scene.content.unwrap(), AREA, 40.0);
    bencher.bench(|| plan(black_box(&scene), camera, AREA).triangles);
}
