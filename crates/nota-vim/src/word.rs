//! Motions de palabra con la misma lógica que `fwd_word`, `bck_word` y
//! `end_word` de vim (`textobject.c`), para que los bordes coincidan.

use crate::cursor::{Class, Cursor, Step};

/// `w`/`W`. Con `stop_at_eol` (operador pendiente) la última palabra no
/// cruza de línea. `false` si empezó en el último carácter del texto.
pub(crate) fn forward(cursor: &mut Cursor, count: usize, big: bool, stop_at_eol: bool) -> bool {
    for remaining in (0..count).rev() {
        let stop_here = stop_at_eol && remaining == 0;
        let ends = |step: Step| step == Step::Edge || (stop_here && step.left_line_body());
        let start_class = cursor.class(big);
        let on_last_line = cursor.line() == cursor.last_line();
        let step = cursor.inc();
        if step == Step::Edge || (on_last_line && step.left_line_body()) {
            return false;
        }
        if stop_here && step.left_line_body() {
            return true;
        }
        if start_class != Class::Blank {
            while cursor.class(big) == start_class {
                if ends(cursor.inc()) {
                    return true;
                }
            }
        }
        while cursor.class(big) == Class::Blank && !cursor.on_empty_line() {
            if ends(cursor.inc()) {
                return true;
            }
        }
    }
    true
}

/// `b`/`B`. `false` si ya estaba al inicio del texto.
pub(crate) fn backward(cursor: &mut Cursor, count: usize, big: bool) -> bool {
    for _ in 0..count {
        if cursor.dec() == Step::Edge {
            return false;
        }
        while cursor.class(big) == Class::Blank {
            if cursor.on_empty_line() {
                break;
            }
            if cursor.dec() == Step::Edge {
                return true;
            }
        }
        if cursor.on_empty_line() {
            continue;
        }
        if cursor.skip_class(cursor.class(big), big, false) {
            return true;
        }
        cursor.inc();
    }
    true
}

/// `e`/`E`. Con `stop`, si ya está en el fin de una palabra no salta a la
/// siguiente (lo que necesita `cw`); con `stop_at_empty_line` se detiene en
/// líneas vacías. `false` si chocó con el fin del texto.
pub(crate) fn end(
    cursor: &mut Cursor,
    count: usize,
    big: bool,
    mut stop: bool,
    stop_at_empty_line: bool,
) -> bool {
    for _ in 0..count {
        let start_class = cursor.class(big);
        if cursor.inc() == Step::Edge {
            return false;
        }
        if cursor.class(big) == start_class && start_class != Class::Blank {
            if cursor.skip_class(start_class, big, true) {
                return false;
            }
        } else if !stop || start_class == Class::Blank {
            while cursor.class(big) == Class::Blank {
                if stop_at_empty_line && cursor.on_empty_line() {
                    break;
                }
                if cursor.inc() == Step::Edge {
                    return false;
                }
            }
            if stop_at_empty_line && cursor.on_empty_line() {
                stop = false;
                continue;
            }
            if cursor.skip_class(cursor.class(big), big, true) {
                return false;
            }
        }
        cursor.dec();
        stop = false;
    }
    true
}

/// Retrocede hasta el inicio de la palabra o del bloque de blancos actual,
/// sin salir de la línea.
pub(crate) fn back_in_line(cursor: &mut Cursor, big: bool) {
    let class = cursor.class(big);
    while !cursor.at_line_start() {
        cursor.dec();
        if cursor.class(big) != class {
            cursor.inc();
            break;
        }
    }
}
