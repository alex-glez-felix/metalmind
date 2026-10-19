//! Dónde va una fórmula y cuánto alto pide su línea, sin GPUI: el motor
//! mueve la ventana con estas medidas y la vista pinta con las mismas.
//!
//! El texto de una línea se pinta centrado en su alto, como lo hace GPUI:
//! la línea base cae a `(alto + ascenso - descenso) / 2` desde arriba. Una
//! fórmula en línea se apoya en esa misma línea base; una línea con una
//! fórmula alta crece lo justo para que quepa con el mismo interlineado
//! que el texto.

use metalmind_syntax::MathPlace;

use super::engine::{MAX_SIDE, MathError, Metrics};

/// Las medidas de la letra del editor y el tamaño de las fórmulas, en
/// píxeles lógicos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// El tamaño de la letra de las fórmulas: un em.
    pub size: f32,
    /// Lo que mide una línea de texto.
    pub line_height: f32,
    pub ascent: f32,
    pub descent: f32,
    /// Arriba y abajo de una fórmula de bloque.
    pub block_margin: f32,
}

/// Una fórmula ya medida, en píxeles lógicos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormulaBox {
    pub width: f32,
    /// Sobre la línea base.
    pub height: f32,
    /// Bajo la línea base.
    pub depth: f32,
}

impl FormulaBox {
    #[must_use]
    pub fn total(&self) -> f32 {
        self.height + self.depth
    }
}

impl Geometry {
    /// Lo que mide la fórmula con esta letra. Una más grande que lo que se
    /// puede pintar ([`MAX_SIDE`]) no compone: se ve su fuente y su línea
    /// mide como texto, en vez de un alto sin tope.
    ///
    /// # Errors
    /// Si un lado pasa de [`MAX_SIDE`] o no es un número.
    pub fn formula(&self, metrics: Metrics) -> Result<FormulaBox, MathError> {
        let formula = FormulaBox {
            width: metrics.width * self.size,
            height: metrics.height * self.size,
            depth: metrics.depth * self.size,
        };
        let fits = |side: f32| side.is_finite() && (0.0..=MAX_SIDE).contains(&side);
        if fits(formula.width) && fits(formula.total()) {
            Ok(formula)
        } else {
            Err(MathError::Render(format!(
                "la fórmula mide {:.0}×{:.0} píxeles; el tope es {MAX_SIDE:.0}",
                formula.width,
                formula.total()
            )))
        }
    }

    /// Lo que sobra de una línea de texto arriba y abajo de la letra.
    fn leading(&self) -> f32 {
        half(self.line_height - self.ascent - self.descent).max(0.0)
    }

    /// Lo que tiene que medir la línea de la fórmula o, si va debajo de un
    /// bloque, lo que suma a su línea. En píxeles enteros, hacia arriba.
    #[must_use]
    pub fn rows(&self, formula: FormulaBox, place: MathPlace) -> usize {
        let leading = self.leading();
        let needed = match place {
            MathPlace::Inline => {
                // Con la línea base centrada como la del texto, lo que pide
                // la parte de arriba y lo que pide la de abajo.
                let above = both_sides(formula.height + leading) - self.ascent + self.descent;
                let below = both_sides(formula.depth + leading) + self.ascent - self.descent;
                above.max(below).max(self.line_height)
            }
            MathPlace::Alone => {
                (formula.total() + both_sides(self.block_margin)).max(self.line_height)
            }
            MathPlace::Below { .. } => formula.total() + both_sides(self.block_margin),
        };
        to_rows(needed)
    }

    /// Desde arriba de una línea de `height`, dónde cae la línea base del
    /// texto que GPUI centra en ella.
    #[must_use]
    pub fn baseline(&self, height: f32) -> f32 {
        half(height + self.ascent - self.descent)
    }

    /// Desde arriba de su lugar (la línea, o el espacio que se suma debajo
    /// de un bloque, de alto `height`), dónde va el borde de arriba de la
    /// fórmula.
    #[must_use]
    pub fn top(&self, formula: FormulaBox, place: MathPlace, height: f32) -> f32 {
        match place {
            MathPlace::Inline => self.baseline(height) - formula.height,
            MathPlace::Alone | MathPlace::Below { .. } => half(height - formula.total()),
        }
    }
}

/// La mitad: GPUI centra el texto en el alto de su línea, y las fórmulas
/// se centran igual, en su línea o en el ancho del área.
#[must_use]
pub fn half(value: f32) -> f32 {
    value / 2.0
}

/// Lo mismo a los dos lados: arriba y abajo, o a la izquierda y a la
/// derecha.
#[must_use]
pub fn both_sides(value: f32) -> f32 {
    value * 2.0
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "un alto en píxeles, positivo y de una pantalla"
)]
fn to_rows(pixels: f32) -> usize {
    pixels.ceil().max(1.0) as usize
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Letra de 16 con 12 de ascenso y 4 de descenso, en líneas de 24: la
    /// línea base cae a 16 de arriba y sobran 4 arriba y 4 abajo.
    const GEOMETRY: Geometry = Geometry {
        size: 16.0,
        line_height: 24.0,
        ascent: 12.0,
        descent: 4.0,
        block_margin: 8.0,
    };

    fn formula(height: f32, depth: f32) -> FormulaBox {
        FormulaBox {
            width: 10.0,
            height,
            depth,
        }
    }

    #[test]
    fn una_formula_mas_grande_que_lo_que_se_pinta_no_compone() {
        let huge = Metrics {
            width: 1.0,
            height: 1.0e30,
            depth: 0.0,
        };
        assert!(GEOMETRY.formula(huge).is_err());
        let fits = Metrics {
            width: 1.0,
            height: 1.0,
            depth: 0.5,
        };
        assert!(GEOMETRY.formula(fits).is_ok());
    }

    #[test]
    fn una_formula_del_alto_del_texto_no_agranda_la_linea() {
        assert_eq!(GEOMETRY.rows(formula(12.0, 4.0), MathPlace::Inline), 24);
        assert_eq!(GEOMETRY.rows(formula(8.0, 0.0), MathPlace::Inline), 24);
    }

    #[test]
    fn una_fraccion_agranda_la_linea_lo_justo() {
        // 20 sobre la línea base y 4 de aire arriba: la base baja a 24 y,
        // como GPUI la centra en la línea, la línea mide 40.
        let fraction = formula(20.0, 4.0);
        let rows = GEOMETRY.rows(fraction, MathPlace::Inline);
        assert_eq!(rows, 40);
        let height = 40.0;
        let top = GEOMETRY.top(fraction, MathPlace::Inline, height);
        assert!((top - 4.0).abs() < f32::EPSILON);
        assert!(GEOMETRY.baseline(height) + fraction.depth <= height - 4.0);
    }

    #[test]
    fn un_bloque_suma_su_alto_con_aire_y_va_centrado() {
        let block = formula(30.0, 10.0);
        assert_eq!(GEOMETRY.rows(block, MathPlace::Below { above: 3 }), 56);
        let top = GEOMETRY.top(block, MathPlace::Below { above: 3 }, 56.0);
        assert!((top - 8.0).abs() < f32::EPSILON);
        assert_eq!(GEOMETRY.rows(formula(2.0, 0.0), MathPlace::Alone), 24);
    }
}
