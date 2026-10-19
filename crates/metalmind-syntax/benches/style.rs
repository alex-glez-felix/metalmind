//! El estilo de las líneas para el editor: calcularlo de cero (abrir una
//! nota e ir al final) y lo que cuesta una tecla. Objetivos en
//! `docs/performance.md`. Correr con `cargo bench -p metalmind-syntax --bench style`.

use divan::Bencher;
use metalmind_syntax::{Lines, Styles};

#[path = "../tests/support/styled.rs"]
mod styled;

use styled::{NoteLines, StyledNote};

fn main() {
    divan::main();
}

#[divan::bench(args = [5_000, 50_000])]
fn estilo_de_cero(bencher: Bencher, lines: usize) {
    let note = NoteLines::new(&metalmind_perf::note(lines));
    bencher.bench(|| {
        let mut styles = Styles::new(note.line_count());
        styles.ensure(usize::MAX, &note);
        styles
    });
}

#[divan::bench(args = [5_000, 50_000])]
fn tecla_en_un_parrafo(bencher: Bencher, lines: usize) {
    let note = StyledNote::new(&metalmind_perf::note(lines));
    bencher
        .with_inputs(|| note.clone())
        .bench_local_refs(StyledNote::type_in_the_middle);
}

#[divan::bench(args = [5_000, 50_000])]
fn abrir_una_cerca_arriba(bencher: Bencher, lines: usize) {
    let note = StyledNote::new(&metalmind_perf::note(lines));
    bencher
        .with_inputs(|| note.clone())
        .bench_local_refs(StyledNote::open_a_fence_at_the_top);
}

/// El estilo de una nota con 200 fórmulas, de cero: lo que se paga al
/// abrirla.
#[divan::bench]
fn estilo_de_200_formulas(bencher: Bencher) {
    let note = NoteLines::new(&metalmind_perf::math_note(200));
    bencher.bench(|| {
        let mut styles = Styles::new(note.line_count());
        styles.ensure(usize::MAX, &note);
        styles
    });
}

/// Un párrafo de 5 k líneas sin ninguna en blanco: una tecla repinta todo
/// el párrafo, porque el código inline de una línea puede cerrar en otra.
#[divan::bench]
fn tecla_en_un_parrafo_de_5k(bencher: Bencher) {
    let note = StyledNote::new(&long_paragraph(5_000));
    bencher
        .with_inputs(|| note.clone())
        .bench_local_refs(StyledNote::type_in_the_middle);
}

fn long_paragraph(lines: usize) -> String {
    (0..lines)
        .map(|i| format!("línea {i} con **negrita** y [[link-{i}]] #tag"))
        .collect::<Vec<_>>()
        .join("\n")
}
