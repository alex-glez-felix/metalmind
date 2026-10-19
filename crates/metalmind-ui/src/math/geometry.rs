//! Dónde va una fórmula y cuánto alto pide su línea, sin GPUI: el motor
//! mueve la ventana con estas medidas y la vista pinta con las mismas.
//!
//! El texto de una línea se pinta centrado en su alto, como lo hace GPUI:
//! la línea base cae a `(alto + ascenso - descenso) / 2` desde arriba. Una
//! fórmula en línea se apoya en esa misma línea base; una línea con una
//! fórmula alta crece lo justo para que quepa con el mismo interlineado
//! que el texto.

use metalmind_syntax::MathPlace;

use super::engine::Metrics;

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
    #[must_use]
    pub fn formula(&self, metrics: Metrics) -> FormulaBox {
        FormulaBox {
            width: metrics.width * self.size,
            height: metrics.height * self.size,
            depth: metrics.depth * self.size,
        }
    }

    /// Lo que sobra de una línea de texto arriba y abajo de la letra.
    fn leading(&self) -> f32 {
        ((self.line_height - self.ascent - self.descent) / 2.0).max(0.0)
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
                let above = 2.0 * (formula.height + leading) - self.ascent + self.descent;
                let below = 2.0 * (formula.depth + leading) + self.ascent - self.descent;
                above.max(below).max(self.line_height)
            }
            MathPlace::Alone => (formula.total() + 4.0 * leading).max(self.line_height),
            MathPlace::Below { .. } => formula.total() + 4.0 * leading,
        };
        to_rows(needed)
    }

    /// Desde arriba de una línea de `height`, dónde cae la línea base del
    /// texto que GPUI centra en ella.
    #[must_use]
    pub fn baseline(&self, height: f32) -> f32 {
        (height + self.ascent - self.descent) / 2.0
    }

    /// Desde arriba de su lugar (la línea, o el espacio que se suma debajo
    /// de un bloque, de alto `height`), dónde va el borde de arriba de la
    /// fórmula.
    #[must_use]
    pub fn top(&self, formula: FormulaBox, place: MathPlace, height: f32) -> f32 {
        match place {
            MathPlace::Inline => self.baseline(height) - formula.height,
            MathPlace::Alone | MathPlace::Below { .. } => (height - formula.total()) / 2.0,
        }
    }
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
    };

    fn formula(height: f32, depth: f32) -> FormulaBox {
        FormulaBox {
            width: 10.0,
            height,
            depth,
        }
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
