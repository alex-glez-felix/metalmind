//! El finder busca en cada tecla: lineal en la cantidad de rutas. Ver
//! `docs/performance.md`.

use nota_perf::{assert_linear, vault_paths};
use nota_search::PathMatcher;

#[test]
fn fuzzy_search_is_linear_in_the_paths() {
    let mut matcher = PathMatcher::new();
    assert_linear(8_000, vault_paths, |paths| {
        matcher.search("tema nota", &paths, 50).len()
    });
}
