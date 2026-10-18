//! Lo que recorre todo el buffer (`/`, `:s`, una macro con count) es lineal;
//! una tecla en Insert no depende del tamaño de la nota. Ver
//! `docs/performance.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use nota_perf::{MISSING_WORD, Session, WORD, assert_flat, assert_linear, huge_line, note};

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
fn typing_in_insert_does_not_depend_on_the_note_size() {
    let word = "palabra ".repeat(40);
    assert_flat(
        5_000,
        |lines| session_after(lines, "i"),
        |session| feed(session, &word),
    );
}

#[test]
fn a_search_without_matches_is_linear() {
    let keys = format!("/{MISSING_WORD}\n");
    assert_linear(1_000, session, |session| feed(session, &keys));
}

#[test]
fn substitute_in_every_line_is_linear() {
    let keys = format!(":%s/{WORD}/apunte/g\n");
    assert_linear(200, session, |session| feed(session, &keys));
}

#[test]
fn substitute_in_one_huge_line_is_linear() {
    let keys = format!(":s/{WORD}/apunte/g\n");
    assert_linear(
        20_000,
        |bytes| Session::new(&huge_line(bytes), 0),
        |session| feed(session, &keys),
    );
}

#[test]
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
