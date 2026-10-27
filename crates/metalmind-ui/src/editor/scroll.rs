//! Posición de scroll en líneas, no en píxeles: con un millón de líneas la
//! altura total pasa de 2^24 px y un `f32` ya no distingue un píxel.

/// Dónde dejó el motor la ventana: su primera línea y lo que oculta arriba
/// de ella si es más alta que la ventana (`Viewport::skip`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anchor {
    pub top: usize,
    pub skip: usize,
}

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

    /// El scroll para pintar si el motor dejó la ventana en `anchor`, con
    /// `hidden` de su primera línea oculto arriba, como fracción de su alto.
    /// Solo se conserva la fracción de la rueda si el motor sigue donde se
    /// le dejó (`synced`); si no, se alinea a sus filas, que si no contaría
    /// como visible una fila recortada abajo, y mostraría otra fila de una
    /// línea alta que la del cursor.
    pub fn follow(self, anchor: Anchor, hidden: f64, synced: Option<Anchor>) -> Self {
        if synced == Some(anchor) {
            self
        } else {
            Self::at(to_f64(anchor.top) + hidden.clamp(0.0, 1.0 - f64::EPSILON))
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

/// Dónde deja la rueda al motor con la vista en `scroll` y el motor en
/// `engine`: en la primera línea entera, sin pasar de la última (con parte
/// de ella oculta, sigue siendo la primera), y en la misma línea, con las
/// filas que el motor ya ocultaba: la rueda las recorre sin moverlo.
pub fn wheel_anchor(scroll: Scroll, last_line: usize, engine: Anchor) -> Anchor {
    let top = scroll.first_full_line().min(last_line);
    let skip = if engine.top == top { engine.skip } else { 0 };
    Anchor { top, skip }
}

/// Sin `wrap`, a dónde lleva la rueda la ventana que se corrió `left`
/// píxeles al correrla `delta` más, con `rest`, lo que quedó de menos de un
/// píxel de las anteriores: el motor solo guarda píxeles enteros, y si no
/// muchos toques cortos de un trackpad no la moverían. Devuelve `left` y lo
/// que vuelve a quedar.
pub fn sideways(left: usize, rest: f32, delta: f32) -> (usize, f32) {
    let target = (f32_of(left) + rest + delta).max(0.);
    (units(target), target - target.floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(top: f64) -> Scroll {
        Scroll { top }
    }

    #[test]
    fn la_rueda_en_la_ultima_linea_no_pasa_de_ella() {
        let engine = Anchor { top: 10, skip: 60 };
        let anchor = wheel_anchor(at(10.5), 10, engine);
        assert_eq!(
            anchor, engine,
            "la última, a medias, sigue siendo la primera"
        );
        assert_eq!(
            at(10.5).follow(engine, 0.3, Some(anchor)),
            at(10.5),
            "y la vista conserva la fracción de la rueda"
        );
        assert_eq!(
            wheel_anchor(at(4.5), 10, engine),
            Anchor { top: 5, skip: 0 },
            "en otra línea, sin filas ocultas"
        );
    }

    #[test]
    fn los_toques_cortos_de_la_rueda_se_suman() {
        let (mut left, mut rest) = (0, 0.);
        for _ in 0..4 {
            (left, rest) = sideways(left, rest, 0.5);
        }
        assert_eq!(left, 2, "cuatro medios píxeles a la derecha");
        for _ in 0..4 {
            (left, rest) = sideways(left, rest, -0.5);
        }
        assert_eq!(left, 0, "y de vuelta, lo mismo");
        assert_eq!(sideways(3, 0., -10.).0, 0, "no pasa del principio");
    }

    #[test]
    fn primera_linea_entera() {
        assert_eq!(at(10.0).first_full_line(), 10);
        assert_eq!(at(10.25).first_full_line(), 11);
    }

    #[test]
    fn la_fraccion_de_la_rueda_dura_mientras_el_motor_no_mueva() {
        let line = |top| Anchor { top, skip: 0 };
        assert_eq!(at(10.25).follow(line(11), 0.0, Some(line(11))), at(10.25));
        assert_eq!(at(10.25).follow(line(14), 0.0, Some(line(11))), at(14.0));
        assert_eq!(at(10.25).follow(line(11), 0.0, None), at(11.0));
        let hidden = Anchor { top: 11, skip: 40 };
        assert_eq!(
            at(10.25).follow(hidden, 0.5, None),
            at(11.5),
            "lo que el motor oculta arriba de una línea alta"
        );
        assert_eq!(
            at(11.25).follow(hidden, 0.5, Some(line(11))),
            at(11.5),
            "en la misma línea, el cursor bajó a otra fila: se sigue al motor"
        );
    }

    #[test]
    fn precision_al_final_de_un_millon_de_lineas() {
        let scroll =
            super::super::rows::scroll_by_pixels(at(999_998.0), 1.0, 1_000_000, 20.0, |_| 20.0);
        assert_eq!(scroll.first_line(), 999_998);
        assert!((scroll.fraction() - 0.05).abs() < 1e-6);
    }
}
