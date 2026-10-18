//! Posición de scroll en líneas, no en píxeles: con un millón de líneas la
//! altura total pasa de 2^24 px y un `f32` ya no distingue un píxel.

/// Primera línea visible, con fracción para el scroll suave.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    top: f64,
}

impl Scroll {
    pub fn first_line(self) -> usize {
        to_line(self.top)
    }

    /// Cuánto de la primera línea queda oculto arriba, en `[0, 1)`.
    pub fn fraction(self) -> f64 {
        self.top.fract()
    }

    pub fn scroll_by(&mut self, lines: f64, last_line: usize) {
        self.top = (self.top + lines).clamp(0.0, to_f64(last_line));
    }

    /// La línea que puso el motor, sin fracción.
    pub fn at_line(line: usize) -> Self {
        Self { top: to_f64(line) }
    }

    /// La primera línea que se ve entera: la de arriba si no hay fracción.
    pub fn first_full_line(self) -> usize {
        self.first_line() + usize::from(self.fraction() > 0.0)
    }
}

pub fn to_f64(line: usize) -> f64 {
    f64::from(u32::try_from(line).unwrap_or(u32::MAX))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "se trunca a propósito y el valor viene acotado a [0, última línea]"
)]
pub fn to_line(value: f64) -> usize {
    value.max(0.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(top: f64) -> Scroll {
        Scroll { top }
    }

    #[test]
    fn scroll_acotado_al_documento() {
        let mut scroll = Scroll::default();
        scroll.scroll_by(-3.0, 100);
        assert_eq!(scroll, at(0.0));
        scroll.scroll_by(2.5, 100);
        assert_eq!((scroll.first_line(), scroll.fraction()), (2, 0.5));
        scroll.scroll_by(1_000.0, 100);
        assert_eq!(scroll, at(100.0));
    }

    #[test]
    fn primera_linea_entera() {
        assert_eq!(at(10.0).first_full_line(), 10);
        assert_eq!(at(10.25).first_full_line(), 11);
        assert_eq!(Scroll::at_line(7), at(7.0));
    }

    #[test]
    fn precision_al_final_de_un_millon_de_lineas() {
        let mut scroll = at(999_998.0);
        scroll.scroll_by(0.05, 1_000_000);
        assert_eq!(scroll.first_line(), 999_998);
        assert!((scroll.fraction() - 0.05).abs() < 1e-6);
    }
}
