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

    /// En la línea `top`, con su fracción.
    pub fn at(top: f64) -> Self {
        Self { top: top.max(0.0) }
    }

    /// El scroll para pintar si el motor dejó la ventana en `top`, con
    /// `skip` de esa línea oculto arriba (una línea más alta que la ventana
    /// con el cursor abajo). Solo se conserva la fracción de la rueda si es
    /// la misma línea que se le dio (`synced`); `None` alinea a las filas
    /// del motor, que si no contaría como visible una fila recortada abajo.
    pub fn follow(self, top: usize, skip: f64, synced: Option<usize>) -> Self {
        if synced == Some(top) {
            self
        } else {
            Self::at(to_f64(top) + skip.clamp(0.0, 1.0 - f64::EPSILON))
        }
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

/// Píxeles del motor, enteros, como los de GPUI.
#[expect(
    clippy::cast_precision_loss,
    reason = "píxeles de una línea: muy por debajo de 2^24"
)]
pub fn f32_of(units: usize) -> f32 {
    units as f32
}

/// Píxeles de GPUI como los del motor: enteros, hacia abajo.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "píxeles de un panel, acotados y nunca negativos"
)]
pub fn units(pixels: f32) -> usize {
    pixels.max(0.).floor() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(top: f64) -> Scroll {
        Scroll { top }
    }

    #[test]
    fn primera_linea_entera() {
        assert_eq!(at(10.0).first_full_line(), 10);
        assert_eq!(at(10.25).first_full_line(), 11);
    }

    #[test]
    fn la_fraccion_de_la_rueda_dura_mientras_el_motor_no_mueva() {
        assert_eq!(at(10.25).follow(11, 0.0, Some(11)), at(10.25));
        assert_eq!(at(10.25).follow(14, 0.0, Some(11)), at(14.0));
        assert_eq!(at(10.25).follow(11, 0.0, None), at(11.0));
        assert_eq!(
            at(10.25).follow(11, 0.5, None),
            at(11.5),
            "lo que el motor oculta arriba de una línea alta"
        );
    }

    #[test]
    fn precision_al_final_de_un_millon_de_lineas() {
        let scroll = super::super::rows::scroll_by_pixels(at(999_998.0), 1.0, 1_000_000, |_| 20.0);
        assert_eq!(scroll.first_line(), 999_998);
        assert!((scroll.fraction() - 0.05).abs() < 1e-6);
    }
}
