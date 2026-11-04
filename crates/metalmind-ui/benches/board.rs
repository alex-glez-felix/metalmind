//! La pizarra sin ventana: leerla, teselarla y decidir qué se pinta (09 §3.2).
//! `cargo bench -p metalmind-ui --bench board`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

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
