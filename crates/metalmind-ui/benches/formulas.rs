//! Lo que cuestan las fórmulas de una nota de 200 (`metalmind_perf::math_note`):
//! medirlas en el hilo de la UI, con la caché fría y caliente, y pintarlas en
//! el fondo. Objetivos en `docs/performance.md`.
//!
//!   cargo bench -p metalmind-ui --bench formulas

use divan::Bencher;
use metalmind_perf::FORMULAS;
use metalmind_syntax::{Lines, Styles, TextLines};
use metalmind_ui::math::{Look, MetricsCache, render};

const COUNT: usize = 200;

fn main() {
    divan::main();
}

/// El LaTeX de las fórmulas de la nota, tal como las ve el editor.
fn latex() -> Vec<(String, bool)> {
    let text = metalmind_perf::math_note(COUNT);
    let lines = TextLines::new(&text);
    let mut styles = Styles::new(lines.line_count());
    styles.ensure(usize::MAX, &lines);
    (0..lines.line_count())
        .filter_map(|line| styles.line(line))
        .flat_map(|style| {
            style
                .math
                .iter()
                .map(|spot| (spot.latex.clone(), spot.display))
        })
        .collect()
}

/// Lo que pasa al abrir la nota: cada fórmula se mide por primera vez.
#[divan::bench]
fn medir_200_en_frio(bencher: Bencher) {
    let formulas = latex();
    bencher
        .with_inputs(MetricsCache::default)
        .bench_local_values(|cache| {
            for (latex, display) in &formulas {
                let _ = cache.get(latex, *display);
            }
            cache
        });
}

/// Lo que pasa en cada frame después: todas ya están en la caché.
#[divan::bench]
fn medir_200_en_caliente(bencher: Bencher) {
    let formulas = latex();
    let cache = MetricsCache::default();
    for (latex, display) in &formulas {
        let _ = cache.get(latex, *display);
    }
    bencher.bench_local(|| {
        for (latex, display) in &formulas {
            let _ = cache.get(latex, *display);
        }
    });
}

/// Pintar una de las 22, como en el executor de fondo.
#[divan::bench(args = [1.0, 2.0])]
fn pintar_las_22(bencher: Bencher, scale: f32) {
    let look = Look {
        size: 16.0 * 1.21,
        scale,
        color: [0.85, 0.85, 0.88, 1.0],
        padding: 1.0,
    };
    bencher.bench_local(|| {
        for formula in FORMULAS {
            let _ = render(formula.latex, formula.display, look);
        }
    });
}
