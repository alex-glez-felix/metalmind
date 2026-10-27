//! Lo que 09 §2 y §4 comparan sin abrir ventana: bytes y líneas de cada
//! formato, cuánto cambia el diff al mover una caja o agregar un trazo, y
//! cuánto cuesta construir la escena.
//!
//! `cargo run --release --bin medir`

use std::time::{Duration, Instant};

use metalmind_spike_pizarra::escena::{self, Style};
use metalmind_spike_pizarra::formato::{Board, Node, NodeKind};
use metalmind_spike_pizarra::{generar, mapeo, trazo};

/// Líneas que cambian (quitadas + agregadas) y bytes de esas líneas.
fn diff(before: &str, after: &str) -> (usize, usize) {
    similar::TextDiff::from_lines(before, after)
        .iter_all_changes()
        .filter(|c| c.tag() != similar::ChangeTag::Equal)
        .fold((0, 0), |(lines, bytes), c| {
            (lines + 1, bytes + c.value().len())
        })
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples.get(samples.len() / 2).copied().unwrap_or_default()
}

fn time<T>(runs: usize, mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut last = None;
    let samples = (0..runs)
        .map(|_| {
            let start = Instant::now();
            last = Some(f());
            start.elapsed()
        })
        .collect();
    (median(samples), last.unwrap_or_else(&mut f))
}

fn ms(d: Duration) -> String {
    format!("{:.2} ms", d.as_secs_f64() * 1000.0)
}

fn kb(bytes: usize) -> String {
    #[expect(clippy::cast_precision_loss, reason = "tamaños de archivo")]
    let kb = bytes as f64 / 1024.0;
    format!("{kb:.0} KB")
}

fn stroke(id: &str, x: i32, y: i32) -> Node {
    #[expect(clippy::cast_precision_loss, reason = "80 puntos")]
    let points = (0..80)
        .map(|i| trazo::Point {
            x: i as f32 * 3.3,
            y: (i as f32 * 0.4).sin() * 20.0 + 20.0,
            pressure: None,
        })
        .collect();
    Node {
        id: id.to_owned(),
        x,
        y,
        width: 264,
        height: 40,
        color: None,
        kind: NodeKind::Stroke {
            stroke_width: 2.0,
            points,
        },
        extra: serde_json::Map::new(),
    }
}

fn formats(name: &str, board: &Board) {
    let canvas = board.write();
    let excalidraw = mapeo::to_excalidraw(board).write();
    let elements = board.nodes.len() + board.edges.len();
    println!(
        "\n### {name}: {elements} elementos ({} nodos, {} flechas)\n",
        board.nodes.len(),
        board.edges.len()
    );
    println!("| | Bytes | Líneas | Leer |");
    println!("|---|---|---|---|");
    let (t, _) = time(9, || Board::parse(&canvas));
    println!(
        "| `.canvas` | {} | {} | {} |",
        kb(canvas.len()),
        canvas.lines().count(),
        ms(t)
    );
    let (t, _) = time(9, || {
        metalmind_spike_pizarra::excalidraw::Scene::parse(&excalidraw)
    });
    println!(
        "| `.excalidraw` equivalente | {} | {} | {} |",
        kb(excalidraw.len()),
        excalidraw.lines().count(),
        ms(t)
    );

    // Mover la primera caja que tenga flechas.
    let Some(moved) = board
        .nodes
        .iter()
        .find(|n| {
            board
                .edges
                .iter()
                .any(|e| e.from.node == n.id || e.to.node == n.id)
        })
        .map(|n| n.id.clone())
    else {
        return;
    };
    let mut after = board.clone();
    if let Some(node) = after.nodes.iter_mut().find(|n| n.id == moved) {
        node.x += 40;
        node.y += 20;
    }
    let mut scene = mapeo::to_excalidraw(board);
    let moved_ok = scene
        .move_with_bindings(&moved, 40.0, 20.0, 1_761_500_000_000)
        .is_ok();
    let (cl, cb) = diff(&canvas, &after.write());
    let (el, eb) = diff(&excalidraw, &scene.write());
    let arrows = board
        .edges
        .iter()
        .filter(|e| e.from.node == moved || e.to.node == moved)
        .count();
    println!(
        "\nMover `{moved}` (con {arrows} flechas pegadas) 40 px a la derecha y 20 abajo{}:\n",
        if moved_ok {
            ""
        } else {
            " (no se pudo en .excalidraw)"
        }
    );
    println!("| | Líneas del diff | Bytes del diff |");
    println!("|---|---|---|");
    println!("| `.canvas` | {cl} | {cb} |");
    println!("| `.excalidraw` | {el} | {eb} |");

    // Agregar un trazo de 80 puntos.
    let new_stroke = stroke("trazo-nuevo", 0, -200);
    let mut with_stroke = board.clone();
    with_stroke.nodes.push(new_stroke.clone());
    let (il, ib) = diff(&canvas, &with_stroke.write());
    let mut scene = mapeo::to_excalidraw(board);
    mapeo::push_node(&mut scene, &new_stroke);
    let (xl, xb) = diff(&excalidraw, &scene.write());
    println!("\nAgregar un trazo de 80 puntos:\n");
    println!("| | Líneas del diff | Bytes del diff |");
    println!("|---|---|---|");
    println!("| `.canvas` | {il} | {ib} |");
    println!("| `.excalidraw` | {xl} | {xb} |");
}

fn scene(name: &str, board: &Board) {
    println!("\n### Escena de {name}\n");
    println!(
        "| Estilo | Construir todo | Vértices | Malla en memoria | Re-enrutar al mover una caja |"
    );
    println!("|---|---|---|---|---|");
    for (label, style) in [("limpio", Style::Clean), ("a mano (roughr)", Style::Rough)] {
        let (t, built) = time(5, || escena::build(board, style));
        let Ok((nodes, edges)) = built else {
            println!("| {label} | no se pudo teselar | | | |");
            continue;
        };
        let meshes = nodes.iter().chain(&edges).flat_map(|e| &e.pieces);
        let (vertices, bytes) = meshes.fold((0, 0), |(v, b), piece| {
            (
                v + piece.mesh.vertices.len(),
                b + piece.mesh.vertices.len() * 8 + piece.mesh.indices.len() * 4,
            )
        });
        let moved = &board.nodes[0];
        let (reroute, _) = time(9, || {
            let node = escena::node_scene(moved, style);
            let edges: Vec<_> = board
                .edges
                .iter()
                .filter(|e| e.from.node == moved.id || e.to.node == moved.id)
                .filter_map(|e| {
                    metalmind_spike_pizarra::geometria::route(board, e)
                        .map(|g| escena::edge_scene(e, &g, style))
                })
                .collect();
            (node, edges)
        });
        println!(
            "| {label} | {} | {vertices} | {} | {:.0} µs |",
            ms(t),
            kb(bytes),
            reroute.as_secs_f64() * 1e6
        );
    }
}

fn main() {
    println!("## Formatos");
    formats("el ejemplo", &generar::example());
    for n in [500, 2000] {
        formats(&format!("tablero de {n}"), &generar::board(n, 1));
    }
    println!("\n## Escena");
    for n in [500, 2000] {
        scene(&format!("{n} elementos"), &generar::board(n, 1));
    }
}
