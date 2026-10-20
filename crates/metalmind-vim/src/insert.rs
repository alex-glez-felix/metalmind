use metalmind_buffer::{Bias, Buffer, ByteOffset, Edit, Selection, Selections};

use crate::columns::Columns;
use crate::dot::{BlockChange, Change, Dot};
use crate::layout::Layout;
use crate::lines::first_non_blank;
use crate::markdown::{After, LazyOutline, Newline, opened_item, renumber_items};
use crate::{InsertAt, VimError};

/// Cómo escriben Insert, `o` y `O` una línea nueva.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Typing {
    pub continue_lists: bool,
    /// Para saber qué ítems son hermanos al renumerar.
    pub columns: Columns,
}

impl Typing {
    /// Sin continuar listas si `list` es falso.
    pub(crate) fn lists_if(self, list: bool) -> Self {
        Self {
            continue_lists: self.continue_lists && list,
            ..self
        }
    }
}

/// Lo pegado fuera de Insert: un paso propio de undo en el cursor
/// principal, que queda sobre el último carácter pegado.
pub(crate) fn paste_at_cursor(
    buffer: &mut Buffer,
    selections: &Selections,
    text: &str,
) -> Result<Selections, VimError> {
    buffer.begin_transaction(selections.clone())?;
    let at = selections.primary().head;
    let map = buffer.apply(vec![Edit::insert(at, text)])?;
    let end = map.map(at, Bias::After);
    let pasted = Selections::single(Selection::cursor(buffer.prev_boundary(end)));
    buffer.commit_transaction(pasted.clone())?;
    Ok(pasted)
}

/// Abre la transacción que dura todo el modo Insert, para que `u` la deshaga
/// entera. Dice también si `o` u `O` abrieron un ítem de lista.
pub(crate) fn enter(
    buffer: &mut Buffer,
    selections: &Selections,
    at: InsertAt,
    typing: Typing,
    layout: Option<&mut Layout>,
) -> Result<(Selections, bool), VimError> {
    let points = insertion_points(buffer, selections, at);
    match at {
        InsertAt::LineBelow => open_line(buffer, selections, &points, true, typing, layout),
        InsertAt::LineAbove => open_line(buffer, selections, &points, false, typing, layout),
        _ => {
            buffer.begin_transaction(points.clone())?;
            Ok((points, false))
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

/// `o`/`O`: el undo vuelve al cursor original, no a la línea nueva. Sobre
/// un ítem de lista, la línea nueva es otro ítem; dice si abrió alguno.
fn open_line(
    buffer: &mut Buffer,
    original: &Selections,
    points: &Selections,
    below: bool,
    typing: Typing,
    layout: Option<&mut Layout>,
) -> Result<(Selections, bool), VimError> {
    buffer.begin_transaction(original.clone())?;
    let outline = LazyOutline::new(layout);
    let opened = points.map_to(|point| {
        let line = buffer.line_of(point.head);
        let item = typing
            .continue_lists
            .then(|| opened_item(buffer, line, below, &outline))
            .flatten();
        (point.head, item.unwrap_or_default())
    });
    let edits = opened
        .iter()
        .map(|(at, item)| {
            let text = if below {
                format!("\n{item}")
            } else {
                format!("{item}\n")
            };
            Edit::insert(*at, text)
        })
        .collect();
    let map = buffer.apply(edits)?;
    let cursors = opened.to_selections(|(at, item)| {
        Selection::cursor(if below {
            map.map(*at, Bias::After)
        } else {
            map.map(*at, Bias::Before) + item.len()
        })
    });
    let list = opened.iter().any(|(_, item)| !item.is_empty());
    let after = opened
        .iter()
        .map(|(_, item)| (!item.is_empty()).then_some(After::Opened));
    Ok((renumber(buffer, cursors, after, typing.columns)?, list))
}

/// Un salto de línea en cada cursor; con `continue_lists`, en un ítem de
/// lista abre el siguiente. `ends` es si un ítem vacío termina la lista.
/// Dice también si siguió o terminó alguna lista.
pub(crate) fn newline(
    buffer: &mut Buffer,
    selections: &Selections,
    typing: Typing,
    ends: bool,
    mut layout: Option<&mut Layout>,
) -> Result<(Selections, bool), VimError> {
    if !typing.continue_lists {
        return Ok((type_text(buffer, selections, "\n")?, false));
    }
    // Lo tecleado antes en este Insert ya cambió el texto.
    if let Some(layout) = layout.as_deref_mut() {
        layout.sync(buffer);
    }
    let outline = LazyOutline::new(layout);
    let plans = selections.map_to(|selection| {
        let head = selection.head;
        let line_start = buffer.line_start(buffer.line_of(head));
        (
            head,
            line_start,
            Newline::at(buffer, head, ends, &outline, typing.columns),
        )
    });
    let mut edits: Vec<Edit> = plans
        .iter()
        .map(|(head, line_start, plan)| match plan {
            Newline::Plain => Edit::insert(*head, "\n"),
            Newline::Continue(item) => Edit::insert(*head, format!("\n{item}")),
            &Newline::End { kept, .. } => {
                let line = buffer.line_of(*head);
                Edit::delete(*line_start + kept..buffer.line_end(line))
            }
        })
        .collect();
    // Dos cursores en el mismo ítem vacío lo borran una sola vez.
    edits.sort_by_key(|edit| edit.range.start);
    edits.dedup();
    let map = buffer.apply(edits)?;
    let cursors = plans.to_selections(|(head, line_start, plan)| {
        Selection::cursor(match plan {
            &Newline::End { kept, .. } => map.map(*line_start + kept, Bias::Before),
            Newline::Plain | Newline::Continue(_) => map.map(*head, Bias::After),
        })
    });
    let list = plans.iter().any(|(_, _, plan)| *plan != Newline::Plain);
    let after = plans.iter().map(|(_, _, plan)| match plan {
        Newline::Continue(_) => Some(After::Opened),
        Newline::End { numbering, .. } => numbering.map(After::Ended),
        Newline::Plain => None,
    });
    Ok((renumber(buffer, cursors, after, typing.columns)?, list))
}

/// Renumera las listas que siguen a la línea de cada cursor que abrió o
/// terminó un ítem (`after`, en el orden de los cursores).
fn renumber(
    buffer: &mut Buffer,
    cursors: Selections,
    after: impl Iterator<Item = Option<After>>,
    columns: Columns,
) -> Result<Selections, VimError> {
    let edited: Vec<(usize, After)> = cursors
        .iter()
        .zip(after)
        .filter_map(|(cursor, after)| Some((buffer.line_of(cursor.head), after?)))
        .collect();
    let edits = renumber_items(buffer, edited, columns);
    if edits.is_empty() {
        return Ok(cursors);
    }
    let map = buffer.apply(edits)?;
    Ok(cursors.map(|cursor| Selection::cursor(map.map(cursor.head, Bias::After))))
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
    /// Texto tal cual, también lo pegado aunque traiga saltos de línea.
    Text(String),
    Backspace,
    /// `Enter`. `list`: si puede continuar o terminar una lista; lo
    /// tecleado lo guarda solo si lo hizo, para que el count y `.` no
    /// conviertan en ítem lo que no lo era.
    Newline {
        list: bool,
    },
}

impl Typed {
    /// Dónde quedan los cursores y lo tecleado tal como se repetirá.
    pub(crate) fn apply(
        &self,
        buffer: &mut Buffer,
        selections: &Selections,
        typing: Typing,
        layout: Option<&mut Layout>,
    ) -> Result<(Selections, Self), VimError> {
        Ok(match self {
            Self::Text(text) => (type_text(buffer, selections, text)?, self.clone()),
            Self::Backspace => (backspace(buffer, selections)?, Self::Backspace),
            &Self::Newline { list } => {
                let typing = typing.lists_if(list);
                let (selections, list) = newline(buffer, selections, typing, true, layout)?;
                (selections, Self::Newline { list })
            }
        })
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
            let change = Change::Insert {
                at: InsertAt::Before,
                list: false,
            };
            self.dot = Dot::new(change, None, None);
            self.cursor_moved = false;
        }
        self.dot.record(typed);
    }

    pub(crate) fn cursor_moved(&mut self) {
        self.cursor_moved = true;
    }

    /// Lo que el count pide repetir al salir con `<Esc>`. Las repeticiones de
    /// `o` y `O` abren cada una su línea debajo de la anterior (en una
    /// lista, otro ítem aunque el anterior quedara vacío); las de `I` y `A`
    /// en un bloque van seguidas en la primera línea.
    pub(crate) fn replay(
        &self,
        buffer: &mut Buffer,
        selections: &Selections,
        typing: Typing,
        layout: &mut Option<Layout>,
    ) -> Result<Selections, VimError> {
        let mut selections = selections.clone();
        let (opens_line, list) = match (&self.dot.change, self.cursor_moved) {
            (&Change::Insert { at, list }, false) => (
                matches!(at, InsertAt::LineBelow | InsertAt::LineAbove),
                list,
            ),
            (Change::Block(BlockChange::Insert { .. }, _), false) => (false, false),
            _ => return Ok(selections),
        };
        let opening = typing.lists_if(list);
        for _ in 1..self.dot.count.unwrap_or(1) {
            if opens_line {
                (selections, _) = newline(buffer, &selections, opening, false, layout.as_mut())?;
            }
            for typed in &self.dot.typed {
                (selections, _) = typed.apply(buffer, &selections, typing, layout.as_mut())?;
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
