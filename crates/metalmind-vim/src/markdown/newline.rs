//! Continuar una lista al partir una línea o abrir una nueva.

use std::collections::{BTreeMap, BTreeSet};

use metalmind_buffer::{Buffer, ByteOffset, Edit};

use super::list::{self, Numbering};
use super::{LazyOutline, ListItem};
use crate::columns::Columns;

/// Lo que hace un salto de línea en `head`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Newline {
    /// Fuera de una lista, en la sangría o el marcador, o en código.
    Plain,
    /// En el texto de un ítem: la línea nueva empieza con este marcador.
    Continue(String),
    /// En un ítem vacío: se borra la línea desde `kept` (lo que es de una
    /// cita queda) y la lista termina ahí; los ítems que le seguían toman su
    /// número.
    End {
        kept: usize,
        numbering: Option<Numbering>,
    },
}

impl Newline {
    /// `ends` es si un ítem vacío termina la lista, como al teclear `Enter`;
    /// si no, también continúa, como al repetir `3o`.
    pub(crate) fn at(
        buffer: &Buffer,
        head: ByteOffset,
        ends: bool,
        outline: &LazyOutline,
        columns: Columns,
    ) -> Self {
        let line = buffer.line_of(head);
        let text = buffer.line_text(line);
        let column = head.get() - buffer.line_start(line).get();
        let Some(item) = ListItem::parse(&text).filter(|item| column >= item.content) else {
            return Self::Plain;
        };
        if outline.get(buffer).is_literal(line) {
            return Self::Plain;
        }
        if ends && item.is_empty() {
            return Self::End {
                kept: item.quote(),
                numbering: item.numbering(columns),
            };
        }
        match item.next_marker() {
            Some(marker) => Self::Continue(item.sibling(marker)),
            None => Self::Plain,
        }
    }
}

/// El inicio del ítem que abre `o` (debajo) u `O` (encima) de `line`, si es
/// un ítem: `O` toma su número, y el renumerado corre los demás.
pub(crate) fn opened_item(
    buffer: &Buffer,
    line: usize,
    below: bool,
    outline: &LazyOutline,
) -> Option<String> {
    let text = buffer.line_text(line);
    let item = ListItem::parse(&text)?;
    if outline.get(buffer).is_literal(line) {
        return None;
    }
    let marker = if below {
        item.next_marker()?
    } else {
        item.marker
    };
    Some(item.sibling(marker))
}

/// Lo que sigue a una línea que se editó: un ítem nuevo, cuyos hermanos de
/// abajo siguen su número, o el fin de una lista, que les deja el suyo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum After {
    Opened,
    Ended(Numbering),
}

/// Renumera las listas que siguen a cada línea de `edited`. Dos ítems de la
/// misma lista la renumeran una sola vez, desde el de arriba.
pub(crate) fn renumber_items(
    buffer: &Buffer,
    edited: impl IntoIterator<Item = (usize, After)>,
    columns: Columns,
) -> Vec<Edit> {
    let mut covered = BTreeSet::new();
    let mut edits = Vec::new();
    for (line, after) in edited.into_iter().collect::<BTreeMap<_, _>>() {
        if covered.contains(&line) {
            continue;
        }
        let numbering = match after {
            After::Ended(numbering) => Some(numbering),
            After::Opened => ListItem::parse(&buffer.line_text(line))
                .and_then(|item| item.numbering(columns))
                .and_then(Numbering::next),
        };
        let Some(numbering) = numbering else { continue };
        let (renumbered, siblings) = list::renumber(buffer, line, numbering, columns);
        covered.extend(siblings);
        edits.extend(renumbered);
    }
    edits
}
