//! El finder busca en cada tecla y la búsqueda de texto lee todo el vault:
//! las dos lineales en lo que recorren. Ver `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use metalmind_perf::{FACTOR, RARE_WORD, WORD, assert_linear, vault_paths};
use metalmind_search::{FuzzyMatcher, Query, TextQuery, TextSearcher};
use tempfile::TempDir;

#[test]
fn fuzzy_search_is_linear_in_the_paths() {
    let mut matcher = FuzzyMatcher::paths();
    let query = Query::new("tema nota");
    assert_linear(8_000, vault_paths, |paths| {
        matcher.rank(&query, &paths).len()
    });
}

fn query(pattern: &str) -> TextQuery {
    TextQuery {
        pattern: pattern.to_owned(),
        ..TextQuery::default()
    }
}

/// Con N notas no se llenan todos los hilos del walk y lineal da menos de ×16,
/// pero un costo cuadrático por nota igual pasa de ×40.
#[test]
fn text_search_is_linear_in_the_notes() {
    const NOTES: usize = 2_000;
    let vault = |notes| {
        let dir = TempDir::new().unwrap();
        metalmind_perf::write_vault(dir.path(), notes).unwrap();
        dir
    };
    let (small, large) = (vault(NOTES), vault(FACTOR * NOTES));
    let query = query(RARE_WORD);
    assert_linear(
        NOTES,
        |notes| {
            if notes == NOTES {
                small.path()
            } else {
                large.path()
            }
        },
        |root| {
            let mut searcher = TextSearcher::new(root);
            searcher.search(&query).unwrap().iter().count()
        },
    );
}

#[test]
fn find_is_linear_in_the_text() {
    let matcher = query(WORD).compile().unwrap();
    assert_linear(4_000, metalmind_perf::note, |text| {
        matcher.find(&text).unwrap().len()
    });
}
