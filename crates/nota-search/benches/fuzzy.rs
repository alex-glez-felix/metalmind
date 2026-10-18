//! Buscar en las 5 100 rutas del vault sintético de `nota-vault/benches/open.rs`:
//! tiene que caber holgado en un frame (16 ms) para hacerse en cada tecla.

use divan::Bencher;
use nota_search::PathMatcher;

/// Lo que cabe en la lista del finder.
const PAGE: usize = 50;

fn main() {
    divan::main();
}

fn synthetic_paths() -> Vec<String> {
    let mut paths = Vec::new();
    for top in 0..10 {
        for sub in 0..10 {
            for note in 0..50 {
                paths.push(format!("area-{top}/tema-{sub}/nota-{note}.md"));
            }
            paths.push(format!("area-{top}/tema-{sub}/imagen.png"));
        }
    }
    paths
}

#[divan::bench(args = ["n", "tema nota", "a3t7n42", "zzz"])]
fn buscar_en_5k_rutas(bencher: Bencher, query: &str) {
    let paths = synthetic_paths();
    let mut matcher = PathMatcher::new();
    bencher.bench_local(|| matcher.search(query, &paths, PAGE));
}
