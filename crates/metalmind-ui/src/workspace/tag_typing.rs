//! El tag que se está escribiendo en Insert y cómo aceptar su
//! autocompletado (05 §3) sin salir de la pasada de Insert.
//!
//! En el cuerpo lo dice el estilo de la línea, que ya deja fuera el código;
//! en el frontmatter, `Scan::tag_at` sobre el frontmatter solo, que sabe en
//! qué campo está. Así no se escanea la nota entera en cada tecla.

use metalmind_buffer::ByteOffset;
use metalmind_syntax::{LineKind, Styles, TagPlace, TagSpot};
use metalmind_vim::{Clipboard, Key, Mode};

use super::{PaneId, PaneView, Workspace, WorkspaceError};

/// El nombre de un tag a medio escribir, con el cursor al final.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedTag {
    pub typed: String,
    pub place: TagPlace,
}

impl Workspace {
    /// El tag que se está escribiendo en `pane`: en Insert, con el cursor
    /// justo después de lo escrito de su nombre. En el YAML, solo un
    /// escalar sin comillas, que se puede reemplazar tecleando.
    ///
    /// # Errors
    /// Si `pane` no existe.
    pub fn typed_tag(&self, pane: PaneId) -> Result<Option<TypedTag>, WorkspaceError> {
        let view = self.view(pane)?;
        if view.mode != Mode::Insert {
            return Ok(None);
        }
        let cursor = view.cursor().get();
        let Some(spot) = spot_at(&view, cursor)?
            .filter(|spot| spot.replace == spot.name && spot.name.end == cursor)
        else {
            return Ok(None);
        };
        let typed = view
            .buffer()
            .text_in(ByteOffset::new(spot.name.start)..ByteOffset::new(cursor))?;
        Ok(Some(TypedTag {
            typed,
            place: spot.place,
        }))
    }

    /// Cambia lo escrito del nombre (`typed`) por `text`, como si se
    /// tecleara: entra en el mismo paso de undo que el resto de la pasada
    /// por Insert y `.` lo repite.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición no cae en el buffer.
    pub fn complete_tag(
        &mut self,
        pane: PaneId,
        typed: &str,
        text: &str,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        for _ in typed.chars() {
            self.feed(pane, Key::Backspace, clipboard)?;
        }
        self.type_text(pane, text, clipboard)?;
        Ok(())
    }
}

/// El tag bajo `cursor`, en bytes del documento. Sin estilo todavía en esa
/// línea, ninguno.
fn spot_at(view: &PaneView<'_>, cursor: usize) -> Result<Option<TagSpot>, WorkspaceError> {
    let buffer = view.buffer();
    let line = buffer.line_of(ByteOffset::new(cursor));
    let Some(styles) = view.document.state().styles() else {
        return Ok(None);
    };
    let Some(style) = styles.line(line) else {
        return Ok(None);
    };
    if style.kind == LineKind::Frontmatter {
        let end = frontmatter_end(view, styles, line);
        let text = buffer.text_in(ByteOffset::ZERO..end)?;
        return Ok(metalmind_syntax::scan(&text).tag_at(cursor));
    }
    let start = buffer.line_start(line).get();
    let shift = |range: std::ops::Range<usize>| range.start + start..range.end + start;
    Ok(style.tag_at(cursor - start).map(|spot| TagSpot {
        name: shift(spot.name),
        replace: shift(spot.replace),
        place: spot.place,
    }))
}

/// Dónde termina el frontmatter que contiene `line`, con su `---` de cierre,
/// hasta donde ya hay estilo.
fn frontmatter_end(view: &PaneView<'_>, styles: &Styles, line: usize) -> ByteOffset {
    let buffer = view.buffer();
    let mut last = line;
    while styles
        .line(last + 1)
        .is_some_and(|style| style.kind == LineKind::Frontmatter)
    {
        last += 1;
    }
    if last >= buffer.last_line() {
        buffer.len()
    } else {
        buffer.line_start(last + 1)
    }
}
