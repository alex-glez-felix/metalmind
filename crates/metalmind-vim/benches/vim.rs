//! Teclas de vim sobre una nota grande, como las recibe el editor: cada
//! benchmark arranca con la nota recién abierta y el cursor a la mitad.
//! Objetivos en `docs/performance.md`. Correr con `cargo bench -p metalmind-vim`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use divan::Bencher;
use metalmind_perf::{MISSING_WORD, Session, WORD};

fn main() {
    divan::main();
}

/// Mide `keys` sobre la nota de `lines` líneas después de teclear `before`.
fn keys(bencher: Bencher, lines: usize, before: &str, keys: &str) {
    keys_in(bencher, lines, Session::new, before, keys);
}

/// Como [`keys`], con la sesión que arma `open`.
fn keys_in(
    bencher: Bencher,
    lines: usize,
    open: fn(&str, usize) -> Session,
    before: &str,
    keys: &str,
) {
    let text = metalmind_perf::note(lines);
    bencher
        .with_inputs(|| {
            let mut session = open(&text, lines / 2);
            session.feed(before).unwrap();
            session
        })
        .bench_local_values(|mut session| {
            session.feed(keys).unwrap();
            session
        });
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 200)]
fn tecla_en_insert(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "i", "x");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 200)]
fn backspace_en_insert(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "Aabc", "\x08");
}

#[divan::bench(sample_size = 1, sample_count = 200)]
fn dd(bencher: Bencher) {
    keys(bencher, 5_000, "", "dd");
}

#[divan::bench(sample_size = 1, sample_count = 200)]
fn ciw(bencher: Bencher) {
    keys(bencher, 5_000, "w", "ciwapunte\x1b");
}

#[divan::bench(sample_size = 1, sample_count = 200)]
fn punto(bencher: Bencher) {
    keys(bencher, 5_000, "wciwapunte\x1bj", ".");
}

#[divan::bench(sample_size = 1, sample_count = 200)]
fn deshacer_ciw(bencher: Bencher) {
    keys(bencher, 5_000, "wciwapunte\x1b", "u");
}

#[divan::bench(sample_size = 1, sample_count = 50)]
fn macro_100_veces(bencher: Bencher) {
    keys(bencher, 5_000, "qa0wciwapunte\x1bjq", "100@a");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 20)]
fn sustituir_todo(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!(":%s/{WORD}/apunte/g\n"));
}

#[divan::bench(sample_size = 1, sample_count = 20)]
fn deshacer_sustituir_todo(bencher: Bencher) {
    keys(bencher, 5_000, &format!(":%s/{WORD}/apunte/g\n"), "u");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 50)]
fn buscar_sin_resultado(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!("/{MISSING_WORD}\n"));
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 50)]
fn buscar_hacia_atras_sin_resultado(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!("?{MISSING_WORD}\n"));
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 50)]
fn buscar_palabra_sin_resultado(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!("/\\<{MISSING_WORD}\\>\n"));
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 50)]
fn sustituir_sin_resultado(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!(":%s/{MISSING_WORD}/x/g\n"));
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 200)]
fn buscar_la_siguiente(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!("/{WORD}\n"));
}

#[divan::bench(sample_size = 1, sample_count = 200)]
fn n_tras_buscar(bencher: Bencher) {
    keys(bencher, 50_000, &format!("/{WORD}\n"), "n");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 20)]
fn global_borrar(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!(":g/{WORD}/d\n"));
}

#[divan::bench(sample_size = 1, sample_count = 20)]
fn deshacer_global_borrar(bencher: Bencher) {
    keys(bencher, 5_000, &format!(":g/{WORD}/d\n"), "u");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 20)]
fn global_mover_arriba(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", ":g/^/m0\n");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 20)]
fn normal_en_cada_linea(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", ":%norm Ax\n");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 20)]
fn ordenar(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", ":sort\n");
}

#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 20)]
fn global_sustituir(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", &format!(":g/{WORD}/s/{WORD}/apunte/\n"));
}

/// `]]` con el estilo de la nota ya calculado, como en el editor.
#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 200)]
fn siguiente_heading(bencher: Bencher, lines: usize) {
    keys_in(bencher, lines, Session::styled, "", "]]");
}

/// `]]` en un motor sin editor, que calcula el estilo de cero.
#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 20)]
fn siguiente_heading_sin_estilo(bencher: Bencher, lines: usize) {
    keys(bencher, lines, "", "]]");
}

/// `Enter` al final de un ítem de lista: lo continúa.
#[divan::bench(args = [5_000, 50_000], sample_size = 1, sample_count = 200)]
fn enter_en_un_item(bencher: Bencher, lines: usize) {
    keys_in(bencher, lines, Session::styled, "/tarea\nA", "\n");
}
