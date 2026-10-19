//! Las unidades de las medidas de un tema, cada una con su rango: lo que
//! se sale de él es un error con posición, no un valor que se recorta.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};

/// Píxeles, de 0 a 2000.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(try_from = "f32")]
pub struct Px(pub f32);

impl TryFrom<f32> for Px {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        in_range(value, 0., 2000., "una medida en píxeles").map(Self)
    }
}

/// Veces el tamaño de la fuente, de 0.5 a 4.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(try_from = "f32")]
pub struct Factor(pub f32);

impl TryFrom<f32> for Factor {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        in_range(value, 0.5, 4., "un tamaño relativo a la fuente").map(Self)
    }
}

/// El grosor de una letra, de 100 a 900 de cien en cien (400 normal, 700 negrita).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u16")]
pub struct Weight(pub u16);

impl TryFrom<u16> for Weight {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, String> {
        if (100..=900).contains(&value) && value.is_multiple_of(100) {
            Ok(Self(value))
        } else {
            Err(format!(
                "un peso va de 100 a 900, de cien en cien; no {value}"
            ))
        }
    }
}

/// Cuántas filas, de 1 a 100.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub struct Count(pub u8);

impl TryFrom<u8> for Count {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, String> {
        if (1..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(format!("una cantidad de filas va de 1 a 100, no {value}"))
        }
    }
}

/// Un tamaño de letra: veces `font.size` y el interlineado, veces el tamaño.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(try_from = "RawTextStyle")]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTextStyle {
    size: Factor,
    line_height: f32,
}

impl TryFrom<RawTextStyle> for TextStyle {
    type Error = String;

    fn try_from(raw: RawTextStyle) -> Result<Self, String> {
        let line_height = in_range(raw.line_height, 1., 3., "un interlineado")?;
        Ok(Self {
            size: raw.size.0,
            line_height,
        })
    }
}

fn in_range(value: f32, min: f32, max: f32, what: &str) -> Result<f32, String> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(format!("{what} va de {min} a {max}, no {value}"))
    }
}

/// Una medida de `rows` o `layout`: píxeles o un paso de `spacing`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Length(pub f32);

/// Cómo se escribe una [`Length`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LengthRef {
    Px(Px),
    Spacing(String),
}

impl<'de> Deserialize<'de> for LengthRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(LengthVisitor)
    }
}

struct LengthVisitor;

impl Visitor<'_> for LengthVisitor {
    type Value = LengthRef;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("píxeles o un paso de `spacing` (`xs`, `sm`, `md`, `lg`)")
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<LengthRef, E> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Px rechaza lo que no cabe en 0–2000"
        )]
        let value = value as f32;
        Px::try_from(value).map(LengthRef::Px).map_err(E::custom)
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<LengthRef, E> {
        let value =
            u16::try_from(value).map_err(|_| E::custom(format!("{value} píxeles es demasiado")))?;
        self.visit_f64(f64::from(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<LengthRef, E> {
        let value =
            u16::try_from(value).map_err(|_| E::custom(format!("{value} no es una medida")))?;
        self.visit_f64(f64::from(value))
    }

    fn visit_str<E: de::Error>(self, name: &str) -> Result<LengthRef, E> {
        Ok(LengthRef::Spacing(name.to_owned()))
    }
}
