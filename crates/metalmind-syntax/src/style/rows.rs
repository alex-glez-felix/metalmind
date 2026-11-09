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

    /// Lo que mide una línea con este estilo en una sola fila.
    #[must_use]
    pub fn height(&self, style: &LineStyle) -> usize {
        let (mut line, mut below) = (self.base(style), 0_usize);
        for (_, formula, rows) in self.formulas(style) {
            if formula.below() {
                below = below.saturating_add(rows);
            } else {
                line = line.max(rows);
            }
        }
        line.saturating_add(below)
    }

    /// Lo que mide cada una de las `rows` filas de una línea partida, con
    /// cada byte en la fila que dice `row_of` (en la app, la de su
    /// [`ScreenLine`](crate::ScreenLine)), y qué fórmulas quedaron partidas:
    /// la fila de una fórmula alta crece con ella, y la de un bloque suma
    /// debajo de la última. La que es lo único de su línea reparte lo que
    /// mide entre todas sus filas, y la que va entre texto, partida, se ve
    /// como fuente y no cuenta. Sin estilo calculado, cada fila mide como
    /// texto.
    #[must_use]
    pub fn line_rows(
        &self,
        style: Option<&LineStyle>,
        rows: usize,
        row_of: impl Fn(usize) -> usize,
    ) -> LineRows {
        let base = style.map_or(self.text, |style| self.base(style));
        let count = rows.max(1);
        let math = style.map_or(&[][..], |style| &style.math);
        let split = math
            .iter()
            .enumerate()
            .filter(|(_, spot)| splits(spot, count, &row_of))
            .map(|(index, _)| index)
            .collect();
        let mut rows = LineRows {
            rows: vec![base; count],
            below: 0,
            split,
        };
        for (index, formula, height) in style.into_iter().flat_map(|style| self.formulas(style)) {
            if formula.below() {
                rows.below = rows.below.saturating_add(height);
            } else if formula.fills_line() {
                rows.spread(height);
            } else if !rows.splits(index)
                && let Some(row) = rows.rows.get_mut(row_of(formula.range.start))
            {
                *row = (*row).max(height);
            }
        }
        rows
    }

    fn base(&self, style: &LineStyle) -> usize {
        match style.kind {
            LineKind::Heading(level) => self
                .headings
                .get(usize::from(level).saturating_sub(1))
                .copied()
                .unwrap_or(self.text),
            _ => self.text,
        }
    }

    /// Las fórmulas de `style` que componen, con su índice en `style.math`
    /// y lo que miden.
    fn formulas<'s>(
        &'s self,
        style: &'s LineStyle,
    ) -> impl Iterator<Item = (usize, &'s MathSpot, usize)> {
        let formulas = self.formulas.as_deref();
        style
            .math
            .iter()
            .enumerate()
            .filter_map(move |(index, formula)| {
                let rows = formulas?.rows(formula)?;
                Some((index, formula, rows))
            })
    }
}

/// Si `spot` ocupa varias de las `rows` filas de su línea: la que es la
/// línea entera, si hay más de una; las demás, si su fuente quedó partida.
fn splits(spot: &MathSpot, rows: usize, row_of: impl Fn(usize) -> usize) -> bool {
    if spot.fills_line() {
        return rows > 1;
    }
    row_of(spot.range.start) != row_of(spot.range.end.saturating_sub(1))
}

/// Lo que mide cada fila de una línea y lo que suma debajo la fórmula de un
/// bloque, en las unidades de [`Rows`], con las fórmulas que ocupan varias
/// filas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRows {
    pub rows: Vec<usize>,
    pub below: usize,
    /// Los índices en `LineStyle::math` de las fórmulas partidas, en orden.
    pub split: Vec<usize>,
}

impl LineRows {
    /// Si la fórmula `formula` (su índice en `LineStyle::math`) ocupa varias
    /// filas: entre texto se ve su fuente, y sola se pinta sobre todas.
    #[must_use]
    pub fn splits(&self, formula: usize) -> bool {
        self.split.binary_search(&formula).is_ok()
    }

    /// Que las filas juntas midan al menos `height`, cada una lo mismo: así
    /// el alto no depende de si se ve la imagen o su fuente.
    fn spread(&mut self, height: usize) {
        let share = height.div_ceil(self.rows.len());
        for row in &mut self.rows {
            *row = (*row).max(share);
        }
    }

    /// Lo que mide la línea entera.
    #[must_use]
    pub fn height(&self) -> usize {
        self.rows.iter().sum::<usize>().saturating_add(self.below)
    }

    /// Dónde empieza la fila `row`, desde arriba de la línea.
    #[must_use]
    pub fn top(&self, row: usize) -> usize {
        self.rows.iter().take(row).sum()
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
    use crate::{Advances, ScreenLine, Styles, TextLines};

    /// Cada fórmula mide lo que dice su LaTeX; `roto` no compone.
    #[derive(Debug)]
    struct ByLatex;

    impl FormulaRows for ByLatex {
        fn rows(&self, formula: &MathSpot) -> Option<usize> {
            formula.latex.trim().parse().ok()
        }
    }

    /// Cada fórmula mide lo que dice la primera palabra de su LaTeX.
    #[derive(Debug)]
    struct ByFirstWord;

    impl FormulaRows for ByFirstWord {
        fn rows(&self, formula: &MathSpot) -> Option<usize> {
            formula.latex.split_whitespace().next()?.parse().ok()
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
    fn cada_fila_mide_lo_suyo_y_el_bloque_suma_debajo() {
        let rows = Rows {
            formulas: Some(Arc::new(ByLatex)),
            ..Rows::uniform(2)
        };
        let text = "uno dos $5$ tres";
        let lines = TextLines::new(text);
        let mut styles = Styles::new(1);
        styles.ensure(0, &lines);
        let style = styles.line(0);
        let screen = ScreenLine::new(text, Advances::monospace(text, 1.0), style, Some(7.0));
        let measured = rows.line_rows(style, screen.rows(), |byte| screen.row_of(byte));
        assert_eq!(
            (measured.rows.as_slice(), measured.below),
            (&[2, 5, 2][..], 0)
        );
        assert_eq!((measured.height(), measured.top(2)), (9, 7));
        assert_eq!(rows.line_rows(None, 3, |_| 0).rows, [2, 2, 2]);
        let split = rows.line_rows(style, 3, |byte| usize::from(byte >= 9));
        assert_eq!(split.rows, [2, 2, 2], "partida, se ve su fuente");
    }

    /// Las filas de una línea de `count` filas, cada una con diez bytes.
    fn rows_of_split(text: &str, count: usize, rows: &Rows) -> Vec<usize> {
        let lines = TextLines::new(text);
        let mut styles = Styles::new(1);
        styles.ensure(0, &lines);
        rows.line_rows(styles.line(0), count, |byte| byte / 10).rows
    }

    /// La línea que es solo una fórmula mide lo mismo se vea la imagen o la
    /// fuente, y el número de filas sigue saliendo de la fuente.
    #[rstest::rstest]
    #[case::bloque_alta("$$ 9       \\quad \\quad$$", 2, &[5, 5])]
    #[case::en_linea_alta("$9       \\quad \\quad$", 2, &[5, 5])]
    #[case::reparte_hacia_arriba("$$ 9       \\quad \\quad \\quad \\quad$$", 4, &[3, 3, 3, 3])]
    #[case::cabe_en_sus_filas("$$ 3       \\quad \\quad$$", 2, &[2, 2])]
    fn una_formula_sola_partida_reparte_su_alto(
        #[case] text: &str,
        #[case] count: usize,
        #[case] expected: &[usize],
    ) {
        let rows = Rows {
            formulas: Some(Arc::new(ByFirstWord)),
            ..Rows::uniform(2)
        };
        assert_eq!(rows_of_split(text, count, &rows), expected);
    }

    /// Con la sangría en su propia fila, la fórmula entera en la siguiente
    /// también es la línea: reparte su alto entre las dos.
    #[test]
    fn una_formula_sola_tras_una_fila_de_sangria_reparte_su_alto() {
        let rows = Rows {
            formulas: Some(Arc::new(ByFirstWord)),
            ..Rows::uniform(2)
        };
        let text = format!("{}$9$", " ".repeat(10));
        assert_eq!(rows_of_split(&text, 2, &rows), [5, 5]);
    }

    /// Qué fórmulas quedaron partidas, con cualquier medidor o sin él: la
    /// vista lo lee de aquí. Una fila vacía no parte lo que la rodea.
    #[rstest::rstest]
    #[case::enteras("a $1$ b $2$", &[], &[])]
    #[case::la_segunda_partida("a $1$ b $2$", &[9], &[1])]
    #[case::fila_vacia_antes("a $1$ b $2$", &[2, 2], &[])]
    #[case::fila_vacia_dentro("a $1$ b $2$", &[3, 3], &[0])]
    #[case::fila_vacia_al_final("a $1$ b $2$", &[11], &[])]
    #[case::sola_en_una_fila("$$1$$", &[], &[])]
    #[case::sola_en_dos_filas("$$1$$", &[5], &[0])]
    fn la_formula_que_ocupa_varias_filas_esta_partida(
        #[case] text: &str,
        #[case] breaks: &[usize],
        #[case] split: &[usize],
    ) {
        let lines = TextLines::new(text);
        let mut styles = Styles::new(1);
        styles.ensure(0, &lines);
        let style = styles.line(0);
        let screen =
            ScreenLine::with_breaks(Advances::monospace(text, 1.0), breaks.iter().copied());
        for rows in [
            Rows::uniform(2),
            Rows {
                formulas: Some(Arc::new(ByLatex)),
                ..Rows::uniform(2)
            },
        ] {
            let measured = rows.line_rows(style, screen.rows(), |byte| screen.row_of(byte));
            assert_eq!(measured.split, split);
        }
    }

    #[test]
    fn una_formula_partida_entre_texto_mide_como_texto() {
        let rows = Rows {
            formulas: Some(Arc::new(ByFirstWord)),
            ..Rows::uniform(2)
        };
        assert_eq!(
            rows_of_split("hola $9       \\quad$ y", 3, &rows),
            [2, 2, 2]
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
