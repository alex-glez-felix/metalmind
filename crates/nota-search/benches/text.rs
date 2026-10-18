//! Buscar texto en los vaults sintéticos de `nota_perf::write_vault`, de 5 k y
//! 50 k notas de ~1.5 KB, y en una nota de 5 k líneas en memoria. Objetivos en
//! `docs/performance.md`; el umbral de `tantivy`, en 03 §4. Correr con
//! `cargo bench -p nota-search --bench text`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use divan::Bencher;
use nota_perf::{MISSING_WORD, RARE_WORD, WORD};
use nota_search::{Syntax, TextQuery, TextSearcher};
use tempfile::TempDir;

const NOTES: [usize; 2] = [5_000, 50_000];

fn main() {
    divan::main();
}

fn query(pattern: &str, syntax: Syntax) -> TextQuery {
    TextQuery {
        pattern: pattern.to_owned(),
        syntax,
        ..TextQuery::default()
    }
}

fn vault(notes: usize) -> TempDir {
    let dir = TempDir::new().unwrap();
    nota_perf::write_vault(dir.path(), notes).unwrap();
    dir
}

fn search_all(bencher: Bencher, notes: usize, query: &TextQuery) {
    let dir = vault(notes);
    bencher.bench(|| {
        let mut searcher = TextSearcher::new(dir.path());
        searcher.search(query).unwrap().iter().count()
    });
}

/// Una nota de cada mil: lo que cuesta recorrer y leer todo el vault.
#[divan::bench(args = NOTES, sample_count = 10, sample_size = 1)]
fn palabra_rara(bencher: Bencher, notes: usize) {
    search_all(bencher, notes, &query(RARE_WORD, Syntax::Literal));
}

/// Casi todas las notas coinciden: suma armar y mandar los resultados.
#[divan::bench(args = NOTES, sample_count = 10, sample_size = 1)]
fn palabra_comun(bencher: Bencher, notes: usize) {
    search_all(bencher, notes, &query(WORD, Syntax::Literal));
}

/// La fecha del frontmatter con una regex sin literal que la acelere.
#[divan::bench(args = NOTES, sample_count = 10, sample_size = 1)]
fn regex_de_fecha(bencher: Bencher, notes: usize) {
    search_all(bencher, notes, &query(r"\d{4}-\d{2}-1\d", Syntax::Regex));
}

/// Empezar, recibir la primera nota y cancelar: lo que bloquea una tecla que
/// reemplaza la búsqueda anterior.
#[divan::bench(args = NOTES, sample_count = 10, sample_size = 1)]
fn empezar_y_cancelar(bencher: Bencher, notes: usize) {
    let dir = vault(notes);
    let query = query(WORD, Syntax::Literal);
    bencher.bench(|| {
        let mut searcher = TextSearcher::new(dir.path());
        let found = searcher.search(&query).unwrap();
        found.recv().unwrap();
        searcher.cancel();
    });
}

/// Una nota abierta de 5 k líneas con cambios sin guardar.
#[divan::bench(args = [WORD, MISSING_WORD])]
fn nota_en_memoria(bencher: Bencher, pattern: &str) {
    let text = nota_perf::note(5_000);
    let matcher = query(pattern, Syntax::Literal).compile().unwrap();
    bencher.bench(|| matcher.find(&text).unwrap().len());
}
