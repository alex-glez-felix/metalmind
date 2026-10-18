use metalmind_buffer::{Bias, Buffer, ByteOffset, Edit, Selection, Selections};

use crate::dot::{BlockChange, Change, Dot};
use crate::lines::first_non_blank;
use crate::{InsertAt, VimError};

/// Abre la transacción que dura todo el modo Insert, para que `u` la deshaga
/// entera.
pub(crate) fn enter(
    buffer: &mut Buffer,
    selections: &Selections,
    at: InsertAt,
) -> Result<Selections, VimError> {
    let points = insertion_points(buffer, selections, at);
    match at {
        InsertAt::LineBelow => open_line(buffer, selections, &points, Bias::After),
        InsertAt::LineAbove => open_line(buffer, selections, &points, Bias::Before),
        _ => {
            buffer.begin_transaction(points.clone())?;
            Ok(points)
        }
    }
}

/// Dónde empieza a escribirse en cada selección.
pub(crate) fn insertion_points(
    buffer: &Buffer,
    selections: &Selections,
    at: InsertAt,
) -> Selections {
    selections.map(|selection| {
        let head = selection.head;
        let line = buffer.line_of(head);
        let offset = match at {
            InsertAt::After if head < buffer.line_end(line) => buffer.next_boundary(head),
            InsertAt::Before | InsertAt::After => head,
            InsertAt::FirstNonBlank => first_non_blank(buffer, line),
            InsertAt::LineEnd | InsertAt::LineBelow => buffer.line_end(line),
            InsertAt::LineAbove => buffer.line_start(line),
        };
        Selection::cursor(offset)
    })
}

/// `o`/`O`: el undo vuelve al cursor original, no a la línea nueva.
fn open_line(
    buffer: &mut Buffer,
    original: &Selections,
    points: &Selections,
    bias: Bias,
) -> Result<Selections, VimError> {
    buffer.begin_transaction(original.clone())?;
    let map = buffer.apply(
        points
            .iter()
            .map(|point| Edit::insert(point.head, "\n"))
            .collect(),
    )?;
    Ok(points.map(|point| Selection::cursor(map.map(point.head, bias))))
}

pub(crate) fn type_text(
    buffer: &mut Buffer,
    selections: &Selections,
    text: &str,
) -> Result<Selections, VimError> {
    let map = buffer.apply(
        selections
            .iter()
            .map(|selection| Edit::insert(selection.head, text))
            .collect(),
    )?;
    Ok(selections.map(|selection| Selection::cursor(map.map(selection.head, Bias::After))))
}

/// Con el `backspace=indent,eol,start` de nvim, borra lo que haya antes,
/// incluido el salto de línea.
pub(crate) fn backspace(
    buffer: &mut Buffer,
    selections: &Selections,
) -> Result<Selections, VimError> {
    let map = buffer.apply(
        selections
            .iter()
            .filter(|selection| selection.head > ByteOffset::ZERO)
            .map(|selection| Edit::delete(buffer.prev_boundary(selection.head)..selection.head))
            .collect(),
    )?;
    Ok(selections.map(|selection| Selection::cursor(map.map(selection.head, Bias::Before))))
}

/// Algo tecleado en Insert, tal como lo repiten el count y `.`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Typed {
    Text(String),
    Backspace,
}

impl Typed {
    pub(crate) fn apply(
        &self,
        buffer: &mut Buffer,
        selections: &Selections,
    ) -> Result<Selections, VimError> {
        match self {
            Self::Text(text) => type_text(buffer, selections, text),
            Self::Backspace => backspace(buffer, selections),
        }
    }
}

/// Una pasada por Insert: el cambio que la abrió y lo tecleado desde
/// entonces. Al salir con `<Esc>` el count lo repite; al cerrarse queda como
/// el cambio que repite `.`.
#[derive(Debug)]
pub(crate) struct Session {
    dot: Dot,
    /// Tras mover el cursor por fuera, el count ya no repite nada y lo que se
    /// teclee empieza otro cambio, como un `i` ahí (`stop_arrow` de vim).
    cursor_moved: bool,
}

impl Session {
    pub(crate) fn new(dot: Dot) -> Self {
        Self {
            dot,
            cursor_moved: false,
        }
    }

    pub(crate) fn record(&mut self, typed: &Typed) {
        if self.cursor_moved {
            self.dot = Dot::new(Change::Insert(InsertAt::Before), None, None);
            self.cursor_moved = false;
        }
        self.dot.record(typed);
    }

    pub(crate) fn cursor_moved(&mut self) {
        self.cursor_moved = true;
    }

    /// Lo que el count pide repetir al salir con `<Esc>`. Las repeticiones de
    /// `o` y `O` abren cada una su línea debajo de la anterior; las de `I` y
    /// `A` en un bloque van seguidas en la primera línea.
    pub(crate) fn replay(
        &self,
        buffer: &mut Buffer,
        selections: &Selections,
    ) -> Result<Selections, VimError> {
        let mut selections = selections.clone();
        let opens_line = match (&self.dot.change, self.cursor_moved) {
            (&Change::Insert(at), false) => {
                matches!(at, InsertAt::LineBelow | InsertAt::LineAbove)
            }
            (Change::Block(BlockChange::Insert { .. }, _), false) => false,
            _ => return Ok(selections),
        };
        let newline = Typed::Text("\n".to_owned());
        let opened = opens_line.then_some(&newline);
        for _ in 1..self.dot.count.unwrap_or(1) {
            for typed in opened.into_iter().chain(&self.dot.typed) {
                selections = typed.apply(buffer, &selections)?;
            }
        }
        Ok(selections)
    }

    pub(crate) fn into_dot(self) -> Dot {
        self.dot
    }
}

/// Al salir de Insert el cursor queda sobre el último carácter escrito.
pub(crate) fn left_of_cursors(buffer: &Buffer, selections: &Selections) -> Selections {
    selections.map(|selection| {
        let line_start = buffer.line_start(buffer.line_of(selection.head));
        if selection.head > line_start {
            Selection::cursor(buffer.prev_boundary(selection.head))
        } else {
            *selection
        }
    })
}
