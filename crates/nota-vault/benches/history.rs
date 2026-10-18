//! Guardar una nota de 5 000 y 50 000 líneas: lo que espera quien guarda
//! (`Vault::save`) y lo que cuesta su versión en el hilo del vault. Objetivos
//! en `docs/performance.md`. Correr con `cargo bench -p nota-vault --bench history`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::cell::Cell;
use std::fmt::Write as _;
use std::time::SystemTime;

use divan::Bencher;
use nota_config::{HistoryRetention, Paths};
use nota_vault::{History, HistoryOptions, Vault, VaultOptions, VaultPath};
use tempfile::TempDir;

const LINES: [usize; 2] = [5_000, 50_000];

fn main() {
    divan::main();
}

/// La nota con una línea cambiada, distinta en cada llamada.
fn edited(base: &str, edit: usize) -> String {
    let lines: Vec<&str> = base.split_inclusive('\n').collect();
    let at = 7 + (edit * 37) % (lines.len() - 7);
    let mut out = String::with_capacity(base.len() + 16);
    for (n, line) in lines.iter().enumerate() {
        if n == at {
            write!(out, "cambio {edit} ").unwrap();
        }
        out.push_str(line);
    }
    out
}

/// Cada línea distinta, como tras un `:%s` que toca todas.
fn all_changed(base: &str) -> String {
    let mut out = String::with_capacity(base.len() * 2);
    for line in base.split_inclusive('\n') {
        out.push_str("> ");
        out.push_str(line);
    }
    out
}

/// Lo que espera la UI al guardar: la escritura atómica con su `fsync`. La
/// versión se registra después, en el hilo del vault.
#[divan::bench(args = LINES, sample_count = 20, sample_size = 5)]
fn guardar_desde_el_vault(bencher: Bencher, lines: usize) {
    let note = nota_perf::note(lines);
    let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let history = HistoryOptions {
        paths: Paths::under(data.path()),
        retention: HistoryRetention::default(),
    };
    let options = VaultOptions {
        history: Some(history),
        ..VaultOptions::default()
    };
    let (vault, _updates) = Vault::open_with(dir.path(), options).unwrap();
    let path = VaultPath::new("larga.md").unwrap();
    let edit = Cell::new(0);
    bencher
        .with_inputs(|| {
            edit.set(edit.get() + 1);
            edited(&note, edit.get())
        })
        .bench_local_values(|text| vault.save(&path, text).unwrap());
}

#[divan::bench(args = LINES, sample_count = 20)]
fn primera_version(bencher: Bencher, lines: usize) {
    let note = nota_perf::note(lines);
    let path = VaultPath::new("larga.md").unwrap();
    bencher
        .with_inputs(|| TempDir::new().unwrap())
        .bench_local_values(|dir| {
            let history = History::new(dir.path(), HistoryRetention::default());
            history.record(&path, &note, SystemTime::now()).unwrap();
            dir
        });
}

/// Cada guardado cambia una línea sobre un historial que va creciendo, con
/// los snapshots que le toquen en el camino.
#[divan::bench(args = LINES, sample_count = 50, sample_size = 10)]
fn version_con_una_linea_cambiada(bencher: Bencher, lines: usize) {
    let note = nota_perf::note(lines);
    let path = VaultPath::new("larga.md").unwrap();
    let dir = TempDir::new().unwrap();
    let history = History::new(dir.path(), HistoryRetention::default());
    history.record(&path, &note, SystemTime::now()).unwrap();
    let edit = Cell::new(0);
    bencher
        .with_inputs(|| {
            edit.set(edit.get() + 1);
            edited(&note, edit.get())
        })
        .bench_local_values(|text| history.record(&path, &text, SystemTime::now()).unwrap());
}

/// El peor caso del diff: ninguna línea en común con la versión anterior.
#[divan::bench(args = LINES, sample_count = 20)]
fn version_con_todo_cambiado(bencher: Bencher, lines: usize) {
    let note = nota_perf::note(lines);
    let changed = all_changed(&note);
    let path = VaultPath::new("larga.md").unwrap();
    bencher
        .with_inputs(|| {
            let dir = TempDir::new().unwrap();
            let history = History::new(dir.path(), HistoryRetention::default());
            history.record(&path, &note, SystemTime::now()).unwrap();
            (dir, history)
        })
        .bench_local_values(|(dir, history)| {
            history.record(&path, &changed, SystemTime::now()).unwrap();
            dir
        });
}
