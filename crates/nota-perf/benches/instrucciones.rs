//! Los benchmarks clave del presupuesto (`docs/performance.md`), contados en
//! instrucciones con callgrind: deterministas, comparables entre corridas en
//! un runner ruidoso. El CI compara cada PR contra `main` con estos.
//! Correr con `cargo bench -p nota-perf --bench instrucciones`.

#![allow(
    clippy::unwrap_used,
    reason = "en un benchmark, un unwrap que falla es un benchmark roto"
)]

use std::hint::black_box;

use gungraun::prelude::*;
use nota_buffer::{Buffer, Edit, Selection, Selections};
use nota_perf::{MISSING_WORD, Session, WORD};
use nota_search::{PathMatcher, TextMatcher, TextQuery};
use nota_syntax::Scan;

/// La nota de `lines` líneas en Normal, con el cursor a la mitad.
fn session(lines: usize) -> Session {
    Session::new(&nota_perf::note(lines), lines / 2)
}

fn session_after(lines: usize, keys: &str) -> Session {
    let mut session = session(lines);
    session.feed(keys).unwrap();
    session
}

fn note_buffer(lines: usize) -> Buffer {
    Buffer::from(nota_perf::note(lines).as_str())
}

fn cursor(buffer: &Buffer, line: usize) -> Selections {
    Selections::single(Selection::cursor(buffer.line_start(line)))
}

#[library_benchmark]
#[bench::lineas_5k(args = (5_000), setup = note_buffer)]
#[bench::lineas_50k(args = (50_000), setup = note_buffer)]
fn buffer_insertar_un_caracter(mut buffer: Buffer) -> Buffer {
    let at = buffer.line_start(buffer.line_count() / 2);
    buffer.begin_transaction(cursor(&buffer, 0)).unwrap();
    buffer.apply(vec![Edit::insert(at, "x")]).unwrap();
    buffer.commit_transaction(cursor(&buffer, 0)).unwrap();
    black_box(buffer)
}

fn edited_buffer(lines: usize) -> Buffer {
    let mut buffer = note_buffer(lines);
    let at = buffer.line_start(lines / 2);
    buffer.begin_transaction(cursor(&buffer, 0)).unwrap();
    buffer
        .apply(vec![Edit::delete(at..buffer.next_boundary(at))])
        .unwrap();
    buffer.commit_transaction(cursor(&buffer, 0)).unwrap();
    buffer
}

#[library_benchmark]
#[bench::lineas_50k(args = (50_000), setup = edited_buffer)]
fn buffer_deshacer_y_rehacer(mut buffer: Buffer) -> Buffer {
    buffer.undo().unwrap();
    buffer.redo().unwrap();
    black_box(buffer)
}

fn insert_mode(lines: usize) -> Session {
    session_after(lines, "i")
}

fn after_change(lines: usize) -> Session {
    session_after(lines, "wciwapunte\x1bj")
}

fn with_macro(lines: usize) -> Session {
    session_after(lines, "qa0wciwapunte\x1bjq")
}

#[library_benchmark]
#[bench::lineas_5k(args = (5_000), setup = insert_mode)]
#[bench::lineas_50k(args = (50_000), setup = insert_mode)]
fn vim_tecla_en_insert(session: Session) -> Session {
    run(session, "x")
}

#[library_benchmark]
#[bench::dd(args = (5_000, "dd".into()), setup = command)]
#[bench::ciw(args = (5_000, "wciwapunte\x1b".into()), setup = command)]
#[bench::sustituir_todo(args = (5_000, format!(":%s/{WORD}/apunte/g\n")), setup = command)]
#[bench::buscar_sin_resultado_50k(args = (50_000, format!("/{MISSING_WORD}\n")), setup = command)]
fn vim_comando((session, keys): (Session, String)) -> Session {
    run(session, &keys)
}

fn command(lines: usize, keys: String) -> (Session, String) {
    (session(lines), keys)
}

#[library_benchmark]
#[bench::punto(args = (5_000), setup = after_change)]
fn vim_repetir(session: Session) -> Session {
    run(session, ".")
}

#[library_benchmark]
#[bench::macro_100_veces(args = (5_000), setup = with_macro)]
fn vim_macro(session: Session) -> Session {
    run(session, "100@a")
}

fn run(mut session: Session, keys: &str) -> Session {
    session.feed(keys).unwrap();
    black_box(session)
}

#[library_benchmark]
#[bench::nota_5k(args = (5_000), setup = nota_perf::note)]
#[bench::nota_50k(args = (50_000), setup = nota_perf::note)]
#[bench::rachas_de_numeral(args = (5_000), setup = nota_perf::hash_runs)]
#[bench::wikilinks_sin_cerrar(args = (5_000), setup = nota_perf::unclosed_wikilinks)]
#[bench::titulos_sin_cerrar(args = (5_000), setup = nota_perf::unclosed_link_titles)]
#[bench::rachas_de_backticks(args = (2_000), setup = nota_perf::backtick_runs)]
#[bench::linea_de_1_mb(args = (1_000_000), setup = nota_perf::huge_line)]
fn syntax_scan(text: String) -> Scan {
    black_box(nota_syntax::scan(black_box(&text)))
}

#[library_benchmark]
#[bench::rutas_5k(args = (5_000), setup = nota_perf::vault_paths)]
#[bench::rutas_50k(args = (50_000), setup = nota_perf::vault_paths)]
fn search_finder(paths: Vec<String>) -> usize {
    let mut matcher = PathMatcher::new();
    black_box(matcher.search("tema nota", &paths, 50).len())
}

fn note_and_matcher(lines: usize, pattern: &str) -> (String, TextMatcher) {
    let query = TextQuery {
        pattern: pattern.to_owned(),
        ..TextQuery::default()
    };
    (nota_perf::note(lines), query.compile().unwrap())
}

#[library_benchmark]
#[bench::palabra_comun_5k(args = (5_000, WORD), setup = note_and_matcher)]
#[bench::sin_resultado_5k(args = (5_000, MISSING_WORD), setup = note_and_matcher)]
fn search_texto_en_memoria((text, matcher): (String, TextMatcher)) -> usize {
    black_box(matcher.find(black_box(&text)).unwrap().len())
}

library_benchmark_group!(
    name = buffer,
    benchmarks = [buffer_insertar_un_caracter, buffer_deshacer_y_rehacer]
);

library_benchmark_group!(
    name = vim,
    benchmarks = [vim_tecla_en_insert, vim_comando, vim_repetir, vim_macro]
);

library_benchmark_group!(name = syntax, benchmarks = [syntax_scan]);

library_benchmark_group!(
    name = search,
    benchmarks = [search_finder, search_texto_en_memoria]
);

main!(library_benchmark_groups = buffer, vim, syntax, search);
