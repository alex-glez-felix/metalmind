use metalmind_buffer::{Buffer, ByteOffset, Selection};

use crate::columns::Columns;
use crate::find::Find;
use crate::lines::{first_non_blank, is_blank, last_char};
use crate::mark;
use crate::markdown::{LazyOutline, ListItem, indent_of};
use crate::motion::Purpose;
use crate::search::{Query, WordSearch};
use crate::span::{Missed, Region, Span, SpanKind, Target};
use crate::visual::Area;
use crate::{Motion, Operator, TextObject, Viewport};

/// Sobre qué actúa un operador.
#[derive(Clone, Debug)]
pub(crate) enum Object {
    Motion(Motion),
    Find(Find),
    /// `d/foo<CR>`, `dn`: hasta donde empieza la coincidencia.
    Search(Query),
    /// `d*`, `d#`: como `Search`, con la palabra bajo el cursor.
    SearchWord(WordSearch),
    TextObject(TextObject),
    CurrentLine,
    /// El de `>>` y `<<`: como [`Object::CurrentLine`], pero un ítem de
    /// lista lleva consigo a sus hijos.
    ListItems,
    /// `'x` o, si `exact`, `` `x ``, con la mark ya resuelta.
    Mark {
        offset: ByteOffset,
        exact: bool,
    },
    Visual(Area),
}

impl Object {
    /// Lo que abarca `operator` desde `selection`, o dónde queda el cursor si
    /// falla ahí.
    pub(crate) fn target(
        &self,
        buffer: &Buffer,
        selection: &Selection,
        operator: Operator,
        count: Option<usize>,
        viewport: &Viewport,
        outline: &LazyOutline,
    ) -> Result<Target, Missed> {
        let span = match *self {
            Self::Motion(motion) => motion
                .resolve(
                    buffer,
                    selection,
                    count,
                    Purpose::Operate(operator),
                    viewport,
                    outline,
                )
                .ok_or(Missed::Stay)?
                .span_from(selection.head),
            Self::Find(find) => find
                .resolve(buffer, selection.head, count.unwrap_or(1))
                .ok_or(Missed::Stay)?
                .span_from(selection.head),
            Self::Search(ref query) => query
                .find(buffer, selection.head, count.unwrap_or(1))
                .ok_or(Missed::Stay)?
                .destination()
                .span_from(selection.head),
            Self::SearchWord(search) => search
                .find(buffer, selection.head, count.unwrap_or(1))
                .ok_or(Missed::Stay)?
                .destination()
                .span_from(selection.head),
            Self::TextObject(object) => {
                object.resolve(buffer, selection.head, count.unwrap_or(1), outline)?
            }
            Self::Mark { offset, exact } => {
                let kind = if exact {
                    SpanKind::Exclusive
                } else {
                    SpanKind::Linewise
                };
                let destination = mark::destination(buffer, offset, exact);
                Span::between(selection.head, destination, kind)
            }
            Self::CurrentLine => {
                return current_lines(buffer, selection, operator, count.unwrap_or(1))
                    .ok_or(Missed::Stay);
            }
            Self::ListItems => {
                let target = current_lines(buffer, selection, operator, count.unwrap_or(1))
                    .ok_or(Missed::Stay)?;
                return Ok(with_children(buffer, target, viewport.columns(), outline));
            }
            Self::Visual(area) => return Ok(area.target(buffer, selection, viewport.columns())),
        };
        Ok(span.into_target(buffer))
    }
}

/// `3dd` falla en la última línea, pero si hay menos de 3 toma las que haya.
fn current_lines(
    buffer: &Buffer,
    selection: &Selection,
    operator: Operator,
    count: usize,
) -> Option<Target> {
    let first = buffer.line_of(selection.head);
    let last_line = buffer.last_line();
    if count > 1 && first >= last_line {
        return None;
    }
    let last = first.saturating_add(count - 1).min(last_line);
    Some(Target {
        region: Region::Lines { first, last },
        start: line_start_for(buffer, selection, operator, last),
        adjusted: false,
        visual: false,
        nested: false,
    })
}

/// Si la primera línea es un ítem de lista fuera de código y frontmatter,
/// extiende las líneas hasta el último hijo de cada ítem: lo que sigue más
/// indentado, con las líneas en blanco de en medio.
fn with_children(
    buffer: &Buffer,
    target: Target,
    columns: Columns,
    outline: &LazyOutline,
) -> Target {
    let Region::Lines { first, last } = target.region else {
        return target;
    };
    let item = ListItem::parse(&buffer.line_text(first)).is_some();
    if !item || outline.get(buffer).is_literal(first) {
        return target;
    }
    let last = (first..=last).fold(last, |end, line| end.max(last_child(buffer, line, columns)));
    Target {
        region: Region::Lines { first, last },
        nested: true,
        ..target
    }
}

fn last_child(buffer: &Buffer, line: usize, columns: Columns) -> usize {
    let level_of = |line| columns.line_width(indent_of(&buffer.line_text(line)));
    let level = level_of(line);
    let mut last = line;
    for next in line + 1..buffer.line_count() {
        if buffer.line_text(next).chars().all(is_blank) {
            continue;
        }
        if level_of(next) <= level {
            break;
        }
        last = next;
    }
    last
}

/// Como `nv_lineop` de vim: `g~~`, `guu` y `gUU` llevan antes el cursor al
/// primer no blanco de la última línea; `dd`, `cc` e `yy` no lo mueven.
fn line_start_for(
    buffer: &Buffer,
    selection: &Selection,
    operator: Operator,
    last: usize,
) -> ByteOffset {
    if !operator.transforms() {
        return selection.head;
    }
    let destination = first_non_blank(buffer, last).min(last_char(buffer, last));
    selection.head.min(destination)
}
