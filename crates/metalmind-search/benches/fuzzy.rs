//! Buscar en las rutas del vault sintético de `metalmind-vault/benches/open.rs`:
//! tiene que caber holgado en un frame (16 ms) para hacerse en cada tecla.
//! Objetivos en `docs/performance.md`.

use divan::Bencher;
use metalmind_search::{FuzzyMatcher, Query};

/// Las filas que pinta la lista del finder: solo ellas piden posiciones.
const PAGE: usize = 50;

fn main() {
    divan::main();
}

#[divan::bench(args = ["n", "tema nota", "a3t7n42", "zzz"])]
fn buscar_en_5k_rutas(bencher: Bencher, query: &str) {
    search(bencher, 5_000, query);
}

#[divan::bench(args = ["n", "tema nota", "a3t7n42", "zzz"])]
fn buscar_en_50k_rutas(bencher: Bencher, query: &str) {
    search(bencher, 50_000, query);
}

fn search(bencher: Bencher, notes: usize, query: &str) {
    let paths = metalmind_perf::vault_paths(notes);
    let mut matcher = FuzzyMatcher::paths();
    bencher.bench_local(|| {
        let query = Query::new(query);
        let ranked = matcher.rank(&query, &paths);
        for found in ranked.iter().take(PAGE) {
            divan::black_box(matcher.positions(&query, &paths[found.index]));
        }
        ranked
    });
}
