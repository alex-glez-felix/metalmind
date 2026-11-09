//! Abrir un vault sintético de 5 000 y 50 000 notas con links, en carpetas
//! anidadas: walk, watcher e índices, leyendo todas las notas o con la caché
//! al día. Objetivos en
//! `docs/performance.md`. Correr con `cargo bench -p metalmind-vault --bench open`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::fs::{self, File};
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, SystemTime};

use divan::Bencher;
use metalmind_perf::{NoteNames, linked_vault};
use metalmind_vault::{Vault, VaultOptions};
use tempfile::TempDir;

fn main() {
    divan::main();
}

/// `metalmind_perf::linked_vault`, con seis links por nota, en disco y con un
/// mtime viejo: uno reciente no es de fiar y la caché leería la nota.
fn synthetic_vault(root: &Path, notes: usize) {
    for (path, text) in linked_vault(notes, NoteNames::Repeated) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        let mut file = File::create(full).unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(3_600))
            .unwrap();
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
        assert_eq!(vault.snapshot().files.files().count(), notes + 100);
        vault
    });
}

#[divan::bench(sample_count = 20)]
fn abrir_5k_notas_con_cache(bencher: Bencher) {
    open_cached(bencher, 5_000);
}

#[divan::bench(sample_count = 10)]
fn abrir_50k_notas_con_cache(bencher: Bencher) {
    open_cached(bencher, 50_000);
}

fn open_cached(bencher: Bencher, notes: usize) {
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    synthetic_vault(dir.path(), notes);
    let options = VaultOptions {
        cache: Some(data.path().join("notes")),
        ..VaultOptions::default()
    };
    // Al soltarlo, la caché ya está escrita.
    drop(Vault::open_with(dir.path(), options.clone()).unwrap());
    bencher.bench(|| {
        let (vault, _updates) = Vault::open_with(dir.path(), options.clone()).unwrap();
        assert_eq!(vault.snapshot().files.files().count(), notes + 100);
        vault
    });
}
