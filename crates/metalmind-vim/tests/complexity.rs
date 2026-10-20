//! Lo que recorre todo el buffer (`/`, `:s`, una macro con count) es lineal;
//! una tecla en Insert no depende del tamaño de la nota. Ver
//! `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::cell::RefCell;
use std::collections::HashMap;

use metalmind_perf::{MISSING_WORD, Session, WORD, assert_flat, assert_linear, huge_line, note};

fn session(lines: usize) -> Session {
    Session::new(&note(lines), lines / 2)
}

fn session_after(lines: usize, keys: &str) -> Session {
    let mut session = session(lines);
    session.feed(keys).unwrap();
    session
}

fn feed(mut session: Session, keys: &str) -> Session {
    session.feed(keys).unwrap();
    session
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn typing_in_insert_does_not_depend_on_the_note_size() {
    let word = "palabra ".repeat(40);
    assert_flat(
        5_000,
        |lines| session_after(lines, "i"),
        |session| feed(session, &word),
    );
}

/// Arma la nota de cada tamaño una sola vez: con las 80 k líneas que necesita
/// `/` para tardar algo medible, armarla en cada ronda cuesta más que buscar.
fn session_of_a_reused_note() -> impl Fn(usize) -> Session {
    let notes = RefCell::new(HashMap::new());
    move |lines| {
        let mut notes = notes.borrow_mut();
        let text = notes.entry(lines).or_insert_with(|| note(lines));
        Session::new(text, lines / 2)
    }
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn a_search_without_matches_is_linear() {
    let keys = format!("/{MISSING_WORD}\n");
    assert_linear(80_000, session_of_a_reused_note(), |session| {
        feed(session, &keys)
    });
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn substitute_in_every_line_is_linear() {
    let keys = format!(":%s/{WORD}/apunte/g\n");
    assert_linear(200, session, |session| feed(session, &keys));
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn substitute_in_one_huge_line_is_linear() {
    let keys = format!(":s/{WORD}/apunte/g\n");
    assert_linear(
        20_000,
        |bytes| Session::new(&huge_line(bytes), 0),
        |session| feed(session, &keys),
    );
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn undoing_a_substitute_is_linear() {
    let keys = format!(":%s/{WORD}/apunte/g\n");
    assert_linear(
        400,
        |lines| session_after(lines, &keys),
        |session| feed(session, "u"),
    );
}

/// La macro baja una línea por vuelta: n vueltas sobre una nota de 4n líneas.
#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn a_macro_with_a_count_is_linear_in_the_count() {
    assert_linear(
        15,
        |times| {
            (
                session_after(4 * times, "qa0wciwapunte\x1bjq"),
                format!("{times}@a"),
            )
        },
        |(session, keys)| feed(session, &keys),
    );
}

/// Cada `d` de `:g` corre las marcas de las líneas que quedan: con el árbol
/// de Fenwick de `metalmind-buffer` no es cuadrático.
#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn global_delete_is_linear() {
    let keys = format!(":g/{WORD}/d\n");
    assert_linear(400, session, |session| feed(session, &keys));
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn global_move_to_the_top_is_linear() {
    assert_linear(400, session, |session| feed(session, ":g/^/m0\n"));
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn normal_on_every_line_is_linear() {
    assert_linear(400, session, |session| feed(session, ":%norm Ax\n"));
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn sorting_every_line_is_linear() {
    assert_linear(4_000, session, |session| feed(session, ":sort\n"));
}
