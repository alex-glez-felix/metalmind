//! Buscar en las rutas del vault sintético de `nota-vault/benches/open.rs`:
//! tiene que caber holgado en un frame (16 ms) para hacerse en cada tecla.
//! Objetivos en `docs/performance.md`.

use divan::Bencher;
use nota_search::PathMatcher;

/// Lo que cabe en la lista del finder.
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
    let paths = nota_perf::vault_paths(notes);
    let mut matcher = PathMatcher::new();
    bencher.bench_local(|| matcher.search(query, &paths, PAGE));
}
