//! Las unidades del tema en los tipos de GPUI.

use gpui_kit::{FontWeight, Hsla, Pixels, px, rgb};
use metalmind_config::{self as config, Represent};

/// Los grupos del tema con los colores en `Hsla`, las medidas en `Pixels` y
/// los pesos en `FontWeight`. Lo relativo a la fuente (`Factor`, `TextStyle`)
/// no cambia: solo [`Theme`](super::Theme) sabe contra qué tamaño se multiplica.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gpui;

impl config::Units for Gpui {}

impl Represent<config::Color> for Gpui {
    type Value = Hsla;

    fn represent(color: config::Color) -> Hsla {
        Hsla::from(rgb(color.rgb)).opacity(color.alpha)
    }
}

impl Represent<config::Px> for Gpui {
    type Value = Pixels;

    fn represent(value: config::Px) -> Pixels {
        px(value.0)
    }
}

impl Represent<config::Length> for Gpui {
    type Value = Pixels;

    fn represent(value: config::Length) -> Pixels {
        px(value.0)
    }
}

impl Represent<config::Weight> for Gpui {
    type Value = FontWeight;

    fn represent(value: config::Weight) -> FontWeight {
        FontWeight(f32::from(value.0))
    }
}

impl Represent<config::Fraction> for Gpui {
    type Value = f32;

    fn represent(value: config::Fraction) -> f32 {
        value.0
    }
}

impl Represent<config::Count> for Gpui {
    type Value = u8;

    fn represent(value: config::Count) -> u8 {
        value.0
    }
}

impl Represent<config::MarkColumns> for Gpui {
    type Value = usize;

    fn represent(value: config::MarkColumns) -> usize {
        usize::from(value.0)
    }
}

impl Represent<config::Factor> for Gpui {
    type Value = config::Factor;

    fn represent(value: config::Factor) -> config::Factor {
        value
    }
}

impl Represent<config::TextStyle> for Gpui {
    type Value = config::TextStyle;

    fn represent(value: config::TextStyle) -> config::TextStyle {
        value
    }
}
