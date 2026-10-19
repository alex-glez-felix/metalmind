//! Cuánto mide cada línea en pantalla: un heading es más alto que el texto,
//! y una fórmula puede serlo. Lo decide el estilo de markdown de
//! `metalmind-syntax`, que se recalcula por tramos al editar y solo hasta
//! donde se pide.

use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::Arc;

use metalmind_buffer::{Buffer, ByteOffset};
use metalmind_syntax::{LineKind, LineStyle, Lines, MathSpot, Styles};

use crate::{LineHeights, Viewport};

/// Lo que mide una fórmula compuesta. Lo sabe quien la compone (la UI); el
/// motor solo lo usa para saber cuánto mide cada línea, así la ventana se
/// mueve con las mismas medidas con que se pinta.
pub trait FormulaRows: Debug + Send + Sync {
    /// En las unidades del [`crate::Viewport`]: lo que tiene que medir la
    /// línea donde la fórmula reemplaza su fuente o, si va debajo de la
    /// línea que cierra un bloque, lo que suma. `None` si no compone: se ve
    /// su fuente y la línea mide como texto.
    fn rows(&self, formula: &MathSpot) -> Option<usize>;
}

/// Lo que mide una línea de texto, una de cada nivel de heading y las
/// fórmulas, en las unidades del [`crate::Viewport`].
#[derive(Clone, Debug)]
pub struct Rows {
    pub text: usize,
    pub headings: [usize; 6],
    /// Sin él, las fórmulas no cambian el alto de su línea.
    pub formulas: Option<Arc<dyn FormulaRows>>,
}

impl Rows {
    /// Todas de `units`.
    #[must_use]
    pub fn uniform(units: usize) -> Self {
        Self {
            text: units,
            headings: [units; 6],
            formulas: None,
        }
    }

    /// Lo que mide una línea con este estilo.
    fn of(&self, style: &LineStyle) -> usize {
        let base = match style.kind {
            LineKind::Heading(level) => self
                .headings
                .get(usize::from(level).saturating_sub(1))
                .copied()
                .unwrap_or(self.text),
            _ => self.text,
        };
        let Some(formulas) = &self.formulas else {
            return base;
        };
        let (mut line, mut below) = (base, 0);
        for formula in &style.math {
            match formulas.rows(formula) {
                Some(rows) if formula.below() => below += rows,
                Some(rows) => line = line.max(rows),
                None => {}
            }
        }
        line + below
    }
}

/// Las mismas medidas: el mismo medidor de fórmulas, no uno que mida igual.
impl PartialEq for Rows {
    fn eq(&self, other: &Self) -> bool {
        let same_formulas = match (&self.formulas, &other.formulas) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.text == other.text && self.headings == other.headings && same_formulas
    }
}

impl Eq for Rows {}

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
            .map_or(self.rows.text, |style| self.rows.of(style))
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

#[cfg(test)]
mod tests {
    use metalmind_buffer::Buffer;
    use pretty_assertions::assert_eq;

    use super::*;

    /// Cada fórmula mide lo que dice su LaTeX; `roto` no compone.
    #[derive(Debug)]
    struct ByLatex;

    impl FormulaRows for ByLatex {
        fn rows(&self, formula: &MathSpot) -> Option<usize> {
            formula.latex.trim().parse().ok()
        }
    }

    fn heights(text: &str) -> Vec<usize> {
        let buffer = Buffer::from(text);
        let mut layout = Layout::new(&buffer);
        layout.set_rows(Rows {
            formulas: Some(Arc::new(ByLatex)),
            ..Rows::uniform(2)
        });
        layout.ensure(&buffer, usize::MAX);
        (0..buffer.line_count())
            .map(|line| layout.height(line))
            .collect()
    }

    #[test]
    fn una_formula_alta_agranda_su_linea_y_un_bloque_suma_debajo() {
        assert_eq!(
            heights("a $5$ b $1$\n$$\n3\n$$\n$roto$ y $$7$$"),
            [5, 2, 2, 2 + 3, 7]
        );
    }

    #[test]
    fn sin_medidor_las_formulas_miden_como_texto() {
        let buffer = Buffer::from("$$9$$");
        let mut layout = Layout::new(&buffer);
        layout.ensure(&buffer, usize::MAX);
        assert_eq!(layout.height(0), 1);
    }
}
