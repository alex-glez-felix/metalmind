//! Editar un buffer grande: lo que cuesta cada tecla y cada undo antes de
//! que el motor vim o la UI hagan nada. Objetivos en `docs/performance.md`.
//! Correr con `cargo bench -p nota-buffer`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use divan::Bencher;
use nota_buffer::{Buffer, ByteOffset, Edit, Selection, Selections};

const LINES: [usize; 2] = [5_000, 50_000];

fn main() {
    divan::main();
}

fn cursor() -> Selections {
    Selections::single(Selection::cursor(ByteOffset::new(0)))
}

fn edit(buffer: &mut Buffer, edits: Vec<Edit>) {
    buffer.begin_transaction(cursor()).unwrap();
    buffer.apply(edits).unwrap();
    buffer.commit_transaction(cursor()).unwrap();
}

fn middle(buffer: &Buffer) -> ByteOffset {
    buffer.line_start(buffer.line_count() / 2)
}

/// Una inserción al inicio de cada línea, en una sola transacción: lo que
/// arma un `:s` o un cursor por línea.
fn one_insert_per_line(buffer: &Buffer) -> Vec<Edit> {
    (0..buffer.line_count())
        .map(|line| Edit::insert(buffer.line_start(line), "> "))
        .collect()
}

#[divan::bench(args = LINES, sample_size = 1, sample_count = 200)]
fn insertar_un_caracter(bencher: Bencher, lines: usize) {
    let text = nota_perf::note(lines);
    bencher
        .with_inputs(|| Buffer::from(text.as_str()))
        .bench_local_refs(|buffer| {
            let at = middle(buffer);
            edit(buffer, vec![Edit::insert(at, "x")]);
        });
}

#[divan::bench(args = LINES, sample_size = 1, sample_count = 200)]
fn borrar_un_caracter(bencher: Bencher, lines: usize) {
    let text = nota_perf::note(lines);
    bencher
        .with_inputs(|| Buffer::from(text.as_str()))
        .bench_local_refs(|buffer| {
            let at = middle(buffer);
            edit(buffer, vec![Edit::delete(at..buffer.next_boundary(at))]);
        });
}

#[divan::bench(args = LINES, sample_size = 1, sample_count = 200)]
fn deshacer_y_rehacer(bencher: Bencher, lines: usize) {
    let text = nota_perf::note(lines);
    bencher
        .with_inputs(|| {
            let mut buffer = Buffer::from(text.as_str());
            let at = middle(&buffer);
            edit(&mut buffer, vec![Edit::insert(at, "x")]);
            buffer
        })
        .bench_local_refs(|buffer| {
            buffer.undo().unwrap();
            buffer.redo().unwrap();
        });
}

#[divan::bench(args = LINES, sample_size = 1, sample_count = 50)]
fn una_edicion_por_linea(bencher: Bencher, lines: usize) {
    let text = nota_perf::note(lines);
    bencher
        .with_inputs(|| {
            let buffer = Buffer::from(text.as_str());
            let edits = one_insert_per_line(&buffer);
            (buffer, edits)
        })
        .bench_local_values(|(mut buffer, edits)| {
            edit(&mut buffer, edits);
            buffer
        });
}

#[divan::bench(args = LINES, sample_size = 1, sample_count = 50)]
fn deshacer_una_edicion_por_linea(bencher: Bencher, lines: usize) {
    let text = nota_perf::note(lines);
    bencher
        .with_inputs(|| {
            let mut buffer = Buffer::from(text.as_str());
            let edits = one_insert_per_line(&buffer);
            edit(&mut buffer, edits);
            buffer
        })
        .bench_local_refs(|buffer| buffer.undo().unwrap().is_some());
}
