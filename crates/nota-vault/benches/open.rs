//! Abrir un vault sintético de 5 000 notas en carpetas anidadas (objetivo de
//! M2: < 1 s). Correr con `cargo bench -p nota-vault`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::fs;
use std::path::Path;

use divan::Bencher;
use nota_vault::Vault;
use tempfile::TempDir;

const NOTES: usize = 5_000;

fn main() {
    divan::main();
}

/// 10 carpetas × 10 subcarpetas × 50 notas, más un adjunto por subcarpeta.
fn synthetic_vault(root: &Path) {
    for top in 0..10 {
        for sub in 0..10 {
            let dir = root.join(format!("area-{top}/tema-{sub}"));
            fs::create_dir_all(&dir).unwrap();
            for note in 0..NOTES / 100 {
                let body = format!("# Nota {note}\n\nVer [[nota-{}]].\n", note + 1);
                fs::write(dir.join(format!("nota-{note}.md")), body).unwrap();
            }
            fs::write(dir.join("imagen.png"), [0_u8; 64]).unwrap();
        }
    }
}

#[divan::bench(sample_count = 20)]
fn abrir_5k_notas(bencher: Bencher) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path());
    bencher.bench(|| {
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        assert_eq!(vault.snapshot().files().count(), NOTES + 100);
        vault
    });
}
