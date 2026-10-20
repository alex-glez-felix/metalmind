//! Cuánto mide una línea según su estilo: un heading es más alto que el
//! texto, y una fórmula compuesta puede serlo.

use std::fmt::Debug;
use std::sync::Arc;

use super::{LineKind, LineStyle, MathSpot};

/// Lo que mide una fórmula compuesta. Lo sabe quien la compone (la UI);
/// aquí solo cuenta para el alto de su línea, así la ventana se mueve con
/// las mismas medidas con que se pinta.
pub trait FormulaRows: Debug + Send + Sync {
    /// En las unidades de [`Rows`]: lo que tiene que medir la línea donde la
    /// fórmula reemplaza su fuente o, si va debajo de la línea que cierra
    /// un bloque, lo que suma. `None` si no compone: se ve su fuente y la
    /// línea mide como texto.
    fn rows(&self, formula: &MathSpot) -> Option<usize>;
}

/// Lo que mide una línea de texto, una de cada nivel de heading y las
/// fórmulas, en las unidades de quien pinta (en la app, píxeles enteros).
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
    #[must_use]
    pub fn height(&self, style: &LineStyle) -> usize {
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
        let (mut line, mut below) = (base, 0_usize);
        for formula in &style.math {
            match formulas.rows(formula) {
                Some(rows) if formula.below() => below = below.saturating_add(rows),
                Some(rows) => line = line.max(rows),
                None => {}
            }
        }
        line.saturating_add(below)
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{Styles, TextLines};

    /// Cada fórmula mide lo que dice su LaTeX; `roto` no compone.
    #[derive(Debug)]
    struct ByLatex;

    impl FormulaRows for ByLatex {
        fn rows(&self, formula: &MathSpot) -> Option<usize> {
            formula.latex.trim().parse().ok()
        }
    }

    fn heights(text: &str, rows: &Rows) -> Vec<usize> {
        let lines = TextLines::new(text);
        let count = text.split('\n').count();
        let mut styles = Styles::new(count);
        styles.ensure(usize::MAX, &lines);
        (0..count)
            .map(|line| styles.line(line).map_or(0, |style| rows.height(style)))
            .collect()
    }

    #[test]
    fn una_formula_alta_agranda_su_linea_y_un_bloque_suma_debajo() {
        let rows = Rows {
            formulas: Some(Arc::new(ByLatex)),
            ..Rows::uniform(2)
        };
        assert_eq!(
            heights("a $5$ b $1$\n$$\n3\n$$\n$roto$ y $$7$$", &rows),
            [5, 2, 2, 2 + 3, 7]
        );
    }

    #[test]
    fn sin_medidor_las_formulas_miden_como_texto() {
        assert_eq!(heights("$$9$$", &Rows::uniform(1)), [1]);
    }

    #[test]
    fn cada_nivel_de_heading_mide_lo_suyo() {
        let rows = Rows {
            text: 1,
            headings: [3, 2, 1, 1, 1, 1],
            formulas: None,
        };
        assert_eq!(heights("# a\n## b\n### c\nd", &rows), [3, 2, 1, 1]);
    }
}
