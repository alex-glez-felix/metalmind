//! Lo que el motor guarda de cada documento: sus marks `a`–`z` y, si
//! alguien lo pinta, el estilo de cada línea y lo que mide. Quien aloja el
//! motor tiene un [`DocState`] por documento y se lo presta en cada tecla
//! con el [`Target`](crate::Target).

use std::borrow::Cow;

use metalmind_buffer::{Buffer, BufferError, ByteOffset, Point, Splice};
use std::sync::Arc;

use metalmind_syntax::{LineRows, LineStyle, Lines, Rows, Screen, ScreenLine, Styles};

use crate::mark::Marks;
use crate::{LineHeights, Viewport};

#[derive(Debug, Default)]
pub struct DocState {
    pub(crate) marks: Marks,
    /// Sin él, nadie pinta el documento: todas sus líneas miden una fila
    /// y el markdown calcula su estilo de cero cuando lo pide.
    lines: Option<StyledLines>,
    /// Hasta qué reemplazo del buffer ([`Buffer::splice_count`]) lo siguieron
    /// la jumplist del panel y las marks `A`–`Z`.
    followed: usize,
}

/// El estilo de las líneas de un buffer y lo que mide cada una. Se
/// recalcula por tramos al editar y solo hasta donde se pide.
#[derive(Debug)]
struct StyledLines {
    styles: Styles,
    rows: Rows,
}

impl DocState {
    /// Un documento que se pinta con `rows`: guarda el estilo de sus
    /// líneas, que el motor sigue al editar, y la ventana se mueve con lo
    /// que mide cada una.
    #[must_use]
    pub fn painted(buffer: &Buffer, rows: Rows) -> Self {
        Self {
            lines: Some(StyledLines {
                styles: Styles::new(buffer.line_count()),
                rows,
            }),
            ..Self::default()
        }
    }

    /// El estilo de las líneas, para pintarlas; `None` si no se pinta.
    #[must_use]
    pub fn styles(&self) -> Option<&Styles> {
        self.lines.as_ref().map(|lines| &lines.styles)
    }

    /// Devuelve si cambió: entonces las ventanas tienen que revisarse. En
    /// un documento que no se pinta no cambia nada: ahí todo mide una fila.
    pub fn set_rows(&mut self, rows: Rows) -> bool {
        let Some(lines) = self.lines.as_mut() else {
            return false;
        };
        let changed = rows != lines.rows;
        lines.rows = rows;
        changed
    }

    /// Sigue lo editado en `buffer` desde la última vez. Devuelve si hubo
    /// algo: las alturas pueden haber cambiado.
    pub fn sync(&mut self, buffer: &mut Buffer) -> bool {
        let Some(lines) = self.lines.as_mut() else {
            return false;
        };
        let edits = buffer.take_line_edits();
        for edit in &edits {
            lines.styles.edit(edit.first, edit.removed, edit.inserted);
        }
        if lines.styles.line_count() != buffer.line_count() {
            lines.styles.reset(buffer.line_count());
        }
        !edits.is_empty()
    }

    /// Calcula el estilo hasta la línea `through`, si se pinta.
    pub fn ensure(&mut self, buffer: &Buffer, through: usize) {
        if let Some(lines) = self.lines.as_mut() {
            lines.styles.ensure(through, &BufferLines(buffer));
        }
    }

    /// Las marks de `old`, en la misma línea y columna, sobre `buffer`: el
    /// texto releído de `old_buffer`, como hace vim al releer un archivo.
    /// Las de una línea que `buffer` ya no tiene se pierden (vim las guarda
    /// y da `E19`).
    ///
    /// # Errors
    /// Solo si una mark no cae en `old_buffer`, lo que sería un bug.
    pub fn keep_marks_of(
        &mut self,
        old: &Self,
        old_buffer: &Buffer,
        buffer: &mut Buffer,
    ) -> Result<(), BufferError> {
        self.marks = old.marks.carried(old_buffer, buffer)?;
        Ok(())
    }

    /// El estilo, ya al día con `buffer`, para leer el markdown.
    pub(crate) fn styles_mut(&mut self) -> Option<&mut Styles> {
        self.lines.as_mut().map(|lines| &mut lines.styles)
    }

    /// Cómo se ve cada línea de `buffer` en un panel con `screen` (sin él,
    /// ninguna se parte) y filas de texto de `row`: lo que mide y, con
    /// `screen`, dónde cae cada carácter. Si el documento no se pinta, cada
    /// fila mide `row`.
    #[must_use]
    pub fn layout<'a>(
        &'a self,
        buffer: &'a Buffer,
        screen: Option<&'a Screen>,
        row: usize,
    ) -> Layout<'a> {
        Layout {
            lines: self.lines.as_ref(),
            buffer,
            screen,
            row,
        }
    }

    /// Calcula el estilo de lo que mira la ventana de `viewport` con el
    /// cursor en `cursor`: lo de arriba y hasta `extra` líneas más allá de
    /// una ventana bajo la primera línea o el cursor. Ninguna línea mide
    /// menos que una de texto, así que no caben más que esas.
    pub(crate) fn ensure_window(
        &mut self,
        buffer: &Buffer,
        viewport: &Viewport,
        cursor: ByteOffset,
        scrolloff: usize,
        extra: usize,
    ) {
        let rows = viewport.height() / viewport.row().max(1);
        let through = viewport
            .top()
            .max(buffer.line_of(cursor))
            .saturating_add(rows)
            .saturating_add(scrolloff)
            .saturating_add(extra)
            .saturating_add(2);
        self.ensure(buffer, through);
    }

    /// Lo editado en `buffer` que todavía no siguieron la jumplist ni las
    /// marks `A`–`Z`.
    pub(crate) fn unfollowed<'b>(&self, buffer: &'b Buffer) -> &'b [Splice] {
        buffer.splices_since(self.followed)
    }

    /// `point` de la jumplist o de una mark `A`–`Z` tras lo editado en
    /// `buffer` que todavía no siguió, como un rango de `:g` a medio correr.
    pub(crate) fn follow(&self, buffer: &Buffer, point: Point) -> Point {
        self.unfollowed(buffer)
            .iter()
            .fold(point, |point, splice| splice.lines.keep(point))
    }

    /// Ya siguieron todo lo editado en `buffer`.
    pub(crate) fn followed(&mut self, buffer: &Buffer) {
        self.followed = buffer.splice_count();
    }
}

/// Cómo se ven las líneas de un documento en un panel ([`DocState::layout`]).
/// Una línea sin estilo calculado mide como texto: quien mueve la ventana
/// pide antes las que va a mirar.
#[derive(Clone, Copy, Debug)]
pub struct Layout<'a> {
    lines: Option<&'a StyledLines>,
    buffer: &'a Buffer,
    screen: Option<&'a Screen>,
    row: usize,
}

impl Layout<'_> {
    #[must_use]
    pub fn screen(&self) -> Option<&Screen> {
        self.screen
    }

    /// La línea `line` en el ancho del panel; `None` sin pantalla.
    #[must_use]
    pub fn screen_line(&self, line: usize) -> Option<Arc<ScreenLine>> {
        let screen = self.screen?;
        Some(screen.line(&self.buffer.line_text(line), self.style(line)))
    }

    /// Lo que mide cada fila de `line` y lo que suma debajo.
    #[must_use]
    pub fn rows(&self, line: usize) -> LineRows {
        let style = self.style(line);
        let wrapped = self
            .screen
            .filter(|screen| screen.wrap)
            .map(|screen| screen.line(&self.buffer.line_text(line), style));
        let (count, row_of) = match &wrapped {
            Some(screen) => (screen.rows(), Some(screen)),
            None => (1, None),
        };
        let row_of = |byte| row_of.map_or(0, |screen| screen.row_of(byte));
        match self.lines {
            Some(lines) => lines.rows.line_rows(style, count, row_of),
            None => LineRows {
                rows: vec![self.row; count],
                below: 0,
            },
        }
    }

    fn style(&self, line: usize) -> Option<&LineStyle> {
        self.lines?.styles.line(line)
    }
}

impl LineHeights for Layout<'_> {
    fn height(&self, line: usize) -> usize {
        let wraps = self.screen.is_some_and(|screen| screen.wrap);
        match self.lines {
            Some(lines) if !wraps => self
                .style(line)
                .map_or(lines.rows.text, |style| lines.rows.height(style)),
            None if !wraps => self.row,
            _ => self.rows(line).height(),
        }
    }
}

/// Las líneas de un buffer, para `metalmind-syntax`.
pub(crate) struct BufferLines<'b>(pub &'b Buffer);

impl Lines for BufferLines<'_> {
    fn line_count(&self) -> usize {
        self.0.line_count()
    }

    fn line(&self, index: usize) -> Cow<'_, str> {
        self.0.line_text(index)
    }
}
