//! Escanear notas grandes y patológicas. El scan corre en cada cambio de la
//! nota abierta y al indexar el vault. Objetivos en `docs/performance.md`.
//! Correr con `cargo bench -p nota-syntax`.

use divan::Bencher;

fn main() {
    divan::main();
}

fn scan(bencher: Bencher, text: &str) {
    bencher
        .counter(divan::counter::BytesCount::of_str(text))
        .bench(|| nota_syntax::scan(divan::black_box(text)));
}

#[divan::bench(args = [5_000, 50_000])]
fn nota(bencher: Bencher, lines: usize) {
    scan(bencher, &nota_perf::note(lines));
}

#[divan::bench]
fn rachas_de_numeral(bencher: Bencher) {
    scan(bencher, &nota_perf::hash_runs(5_000));
}

#[divan::bench]
fn wikilinks_sin_cerrar(bencher: Bencher) {
    scan(bencher, &nota_perf::unclosed_wikilinks(5_000));
}

#[divan::bench]
fn titulos_sin_cerrar(bencher: Bencher) {
    scan(bencher, &nota_perf::unclosed_link_titles(5_000));
}

#[divan::bench]
fn rachas_de_backticks(bencher: Bencher) {
    scan(bencher, &nota_perf::backtick_runs(2_000));
}

#[divan::bench]
fn linea_de_1_mb(bencher: Bencher) {
    scan(bencher, &nota_perf::huge_line(1_000_000));
}
