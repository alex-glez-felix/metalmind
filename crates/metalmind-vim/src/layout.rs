//! Cuánto mide cada línea en pantalla: un heading es más alto que el texto,
//! y una fórmula puede serlo. Lo decide el estilo de markdown de
//! `metalmind-syntax`, que se recalcula por tramos al editar y solo hasta
//! donde se pide.

use std::borrow::Cow;

use metalmind_buffer::{Buffer, ByteOffset};
use metalmind_syntax::{Lines, Rows, Styles};

use crate::{LineHeights, Viewport};

/// El estilo de las líneas de un buffer y lo que mide cada una. Va con el
/// documento: lo comparten sus paneles.
#[derive(Debug, Clone)]
pub struct Layout {
    styles: Styles,
    rows: Rows,
}

impl Layout {
    #[must_use]
    pub fn new(buffer: &Buffer) -> Self {
        Self {
            styles: Styles::new(buffer.line_count()),
            rows: Rows::uniform(1),
        }
    }

    #[must_use]
    pub fn styles(&self) -> &Styles {
        &self.styles
    }

    #[must_use]
    pub fn rows(&self) -> &Rows {
        &self.rows
    }

    /// Devuelve si cambió: entonces las ventanas tienen que revisarse.
    pub fn set_rows(&mut self, rows: Rows) -> bool {
        let changed = rows != self.rows;
        self.rows = rows;
        changed
    }

    /// Sigue lo editado en `buffer` desde la última vez. Devuelve si hubo
    /// algo: las alturas pueden haber cambiado.
    pub fn sync(&mut self, buffer: &mut Buffer) -> bool {
        let edits = buffer.take_line_edits();
        for edit in &edits {
            self.styles.edit(edit.first, edit.removed, edit.inserted);
        }
        if self.styles.line_count() != buffer.line_count() {
            self.styles.reset(buffer.line_count());
        }
        !edits.is_empty()
    }

    /// El texto es otro (se releyó el archivo): se olvida todo.
    pub fn reset(&mut self, buffer: &mut Buffer) {
        buffer.take_line_edits();
        self.styles.reset(buffer.line_count());
    }

    /// Calcula el estilo hasta la línea `through`.
    pub fn ensure(&mut self, buffer: &Buffer, through: usize) {
        self.styles.ensure(through, &BufferLines(buffer));
    }

    /// Calcula el estilo de lo que mira la ventana de `viewport` con el
    /// cursor en `cursor`: lo de arriba y hasta `extra` líneas más allá de
    /// una ventana bajo la primera línea o el cursor. Ninguna línea mide
    /// menos que una de texto, así que no caben más que esas.
    pub fn ensure_window(
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
}

impl LineHeights for Layout {
    /// Una línea sin estilo calculado mide como texto: quien mueve la
    /// ventana pide antes las que va a mirar.
    fn height(&self, line: usize) -> usize {
        self.styles
            .line(line)
            .map_or(self.rows.text, |style| self.rows.height(style))
    }
}

/// Las líneas de un buffer, para `metalmind-syntax`.
pub struct BufferLines<'b>(pub &'b Buffer);

impl Lines for BufferLines<'_> {
    fn line_count(&self) -> usize {
        self.0.line_count()
    }

    fn line(&self, index: usize) -> Cow<'_, str> {
        self.0.line_text(index)
    }
}
