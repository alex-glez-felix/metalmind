//! Abrir un vault sintético de 5 000 y 50 000 notas con links, en carpetas
//! anidadas: walk, watcher e índice de links. Objetivos en
//! `docs/performance.md`. Correr con `cargo bench -p nota-vault --bench open`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::fs;
use std::path::Path;

use divan::Bencher;
use nota_perf::{NoteNames, linked_vault};
use nota_vault::Vault;
use tempfile::TempDir;

fn main() {
    divan::main();
}

/// `nota_perf::linked_vault`, con seis links por nota, en disco.
fn synthetic_vault(root: &Path, notes: usize) {
    for (path, text) in linked_vault(notes, NoteNames::Repeated) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
}

#[divan::bench(sample_count = 20)]
fn abrir_5k_notas(bencher: Bencher) {
    open(bencher, 5_000);
}

#[divan::bench(sample_count = 10)]
fn abrir_50k_notas(bencher: Bencher) {
    open(bencher, 50_000);
}

fn open(bencher: Bencher, notes: usize) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path(), notes);
    bencher.bench(|| {
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        assert_eq!(vault.snapshot().files().count(), notes + 100);
        vault
    });
}
