//! Adjuntos en un vault sintético de 5 000 y 50 000 notas con links, más
//! 1 000 adjuntos en `adjuntos/` de los que la mitad usa alguna nota:
//! guardar uno pegado y listar los huérfanos. Objetivos en
//! `docs/performance.md`. Correr con `cargo bench -p metalmind-vault --bench attachments`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::cell::Cell;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use divan::Bencher;
use jiff::civil::date;
use metalmind_perf::{NoteNames, linked_vault};
use metalmind_vault::{NewAttachment, Vault, VaultPath};
use tempfile::TempDir;

const ATTACHMENTS: usize = 1_000;

fn main() {
    divan::main();
}

/// `metalmind_perf::linked_vault` en disco; las primeras notas embeben los
/// adjuntos pares.
fn synthetic_vault(root: &Path, notes: usize) {
    let mut files = linked_vault(notes, NoteNames::Repeated);
    // Los adjuntos de `linked_vault` van vacíos.
    let notes = files.iter_mut().filter(|(_, text)| !text.is_empty());
    for (number, (_, text)) in notes.take(ATTACHMENTS / 2).enumerate() {
        let _ = writeln!(text, "![[captura-{}.png]]", number * 2);
    }
    files.extend((0..ATTACHMENTS).map(|n| (format!("adjuntos/captura-{n}.png"), String::new())));
    for (path, text) in files {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }
}

#[divan::bench(args = [5_000, 50_000], sample_count = 20)]
fn huerfanos(bencher: Bencher, notes: usize) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path(), notes);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(vault.orphan_attachments().len(), ATTACHMENTS / 2);
    bencher.bench(|| vault.orphan_attachments());
}

/// Una captura de 200 KB pegada en una nota: escribirla, bajarla a disco,
/// comprobar que el vault la ve y armar su embed.
#[divan::bench(args = [5_000, 50_000], sample_count = 50)]
fn pegar_una_imagen(bencher: Bencher, notes: usize) {
    let dir = TempDir::new().unwrap();
    synthetic_vault(dir.path(), notes);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(200 * 1024, 0);
    let note = VaultPath::new("area-0/tema-0/nota-0.md").unwrap();
    let second = Cell::new(0);
    bencher.bench_local(|| {
        // Otro segundo en cada vuelta: que el nombre no choque con los anteriores.
        let n = second.replace(second.get() + 1);
        let at = date(2026, 10, 18).at(
            i8::try_from(n / 3600 % 24).unwrap(),
            i8::try_from(n / 60 % 60).unwrap(),
            i8::try_from(n % 60).unwrap(),
            0,
        );
        let attachment = NewAttachment {
            bytes: &png,
            note: &note,
            file_name: None,
            at,
        };
        vault.save_attachment(&attachment).unwrap()
    });
}
