use nota_buffer::{Bias, Buffer, ByteOffset, Edit, Selection, Selections};

use crate::dot::{Change, Dot};
use crate::lines::first_non_blank;
use crate::{InsertAt, VimError};

/// Abre la transacción que dura todo el modo Insert, para que `u` la deshaga
/// entera.
pub(crate) fn enter(
    buffer: &mut Buffer,
    selections: &Selections,
    at: InsertAt,
) -> Result<Selections, VimError> {
    let point = |selection: &Selection| {
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
    };
    let points = selections.map(point);
    match at {
        InsertAt::LineBelow => open_line(buffer, selections, &points, Bias::After),
        InsertAt::LineAbove => open_line(buffer, selections, &points, Bias::Before),
        _ => {
            buffer.begin_transaction(points.clone())?;
            Ok(points)
        }
    }
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
            self.dot = Dot::new(Change::Insert(InsertAt::Before), None);
            self.cursor_moved = false;
        }
        self.dot.record(typed);
    }

    pub(crate) fn cursor_moved(&mut self) {
        self.cursor_moved = true;
    }

    /// Lo que el count pide repetir al salir con `<Esc>`. Las repeticiones de
    /// `o` y `O` abren cada una su línea debajo de la anterior.
    pub(crate) fn replay(
        &self,
        buffer: &mut Buffer,
        selections: &Selections,
    ) -> Result<Selections, VimError> {
        let mut selections = selections.clone();
        let (Change::Insert(at), false) = (self.dot.change, self.cursor_moved) else {
            return Ok(selections);
        };
        let newline = Typed::Text("\n".to_owned());
        let opens_line = matches!(at, InsertAt::LineBelow | InsertAt::LineAbove);
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

/// Cierra la transacción y deja el cursor sobre el último carácter escrito.
pub(crate) fn leave(buffer: &mut Buffer, selections: &Selections) -> Result<Selections, VimError> {
    let after = selections.map(|selection| {
        let line_start = buffer.line_start(buffer.line_of(selection.head));
        if selection.head > line_start {
            Selection::cursor(buffer.prev_boundary(selection.head))
        } else {
            *selection
        }
    });
    buffer.commit_transaction(after.clone())?;
    Ok(after)
}
