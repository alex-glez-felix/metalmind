//! Abrir un vault sintético de 5 000 y 50 000 notas en carpetas anidadas.
//! Objetivos en `docs/performance.md`. Correr con `cargo bench -p nota-vault`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::fs;
use std::path::Path;

use divan::Bencher;
use nota_vault::Vault;
use tempfile::TempDir;

fn main() {
    divan::main();
}

/// 10 carpetas × 10 subcarpetas × `notes / 100` notas, más un adjunto por
/// subcarpeta: las rutas de `nota_perf::vault_paths`.
fn synthetic_vault(root: &Path, notes: usize) {
    for top in 0..10 {
        for sub in 0..10 {
            let dir = root.join(format!("area-{top}/tema-{sub}"));
            fs::create_dir_all(&dir).unwrap();
            for note in 0..notes / 100 {
                let body = format!("# Nota {note}\n\nVer [[nota-{}]].\n", note + 1);
                fs::write(dir.join(format!("nota-{note}.md")), body).unwrap();
            }
            fs::write(dir.join("imagen.png"), [0_u8; 64]).unwrap();
        }
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
