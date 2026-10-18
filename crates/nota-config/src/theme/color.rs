//! Colores de un tema: `"#rrggbb"` en la paleta y, en los campos semánticos,
//! un hex, un nombre de la paleta o `{ color = …, alpha = … }`.

use std::fmt;

use serde::Deserialize;
use serde::de::{self, MapAccess, Visitor};

/// Un color sRGB con opacidad aparte, para que `alpha = 0.5` sea exactamente
/// 0.5 y no el 128/255 de un `#rrggbbaa`.
#[derive(Clone, Copy, PartialEq)]
pub struct Color {
    /// `0xrrggbb`.
    pub rgb: u32,
    /// De 0 (transparente) a 1 (opaco).
    pub alpha: f32,
}

impl Color {
    fn parse_hex(text: &str) -> Result<Self, String> {
        let digits = text
            .strip_prefix('#')
            .filter(|digits| digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| format!("`{text}` no es un color `#rrggbb`"))?;
        let rgb = u32::from_str_radix(digits, 16).map_err(|error| error.to_string())?;
        Ok(Self { rgb, alpha: 1. })
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:06x}", self.rgb)?;
        if self.alpha < 1. {
            write!(f, " α{}", self.alpha)?;
        }
        Ok(())
    }
}

/// Un valor de la paleta: siempre un hex opaco.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Hex(pub(crate) Color);

impl TryFrom<String> for Hex {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        Color::parse_hex(&text).map(Self)
    }
}

/// Lo que se escribe en un campo semántico, antes de mirar la paleta.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColorRef {
    pub(crate) base: Base,
    pub(crate) alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Base {
    Literal(Color),
    Palette(String),
}

impl ColorRef {
    fn parse(text: &str, alpha: f32) -> Result<Self, String> {
        if !(0. ..=1.).contains(&alpha) {
            return Err(format!("alpha debe estar entre 0 y 1, no {alpha}"));
        }
        let base = if text.starts_with('#') {
            Base::Literal(Color::parse_hex(text)?)
        } else {
            Base::Palette(text.to_owned())
        };
        Ok(Self { base, alpha })
    }
}

impl<'de> Deserialize<'de> for ColorRef {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ColorRefVisitor)
    }
}

struct ColorRefVisitor;

impl<'de> Visitor<'de> for ColorRefVisitor {
    type Value = ColorRef;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("un color `#rrggbb`, un nombre de la paleta o `{ color = …, alpha = … }`")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<ColorRef, E> {
        ColorRef::parse(text, 1.).map_err(E::custom)
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<ColorRef, M::Error> {
        let (mut color, mut alpha) = (None::<String>, None::<f32>);
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "color" => color = Some(map.next_value()?),
                "alpha" => alpha = Some(map.next_value()?),
                other => return Err(de::Error::unknown_field(other, &["color", "alpha"])),
            }
        }
        let color = color.ok_or_else(|| de::Error::missing_field("color"))?;
        ColorRef::parse(&color, alpha.unwrap_or(1.)).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color_ref(text: &str) -> Result<ColorRef, String> {
        #[derive(Deserialize)]
        struct Field {
            c: ColorRef,
        }
        toml::from_str::<Field>(&format!("c = {text}"))
            .map(|field| field.c)
            .map_err(|error| error.message().to_owned())
    }

    #[test]
    fn hex_nombre_y_tabla_con_alpha() {
        let blue = Color {
            rgb: 0x7a_a2_f7,
            alpha: 1.,
        };
        assert_eq!(
            color_ref("\"#7aa2f7\""),
            Ok(ColorRef {
                base: Base::Literal(blue),
                alpha: 1.
            })
        );
        assert_eq!(
            color_ref("\"azul\""),
            Ok(ColorRef {
                base: Base::Palette("azul".to_owned()),
                alpha: 1.
            })
        );
        assert_eq!(
            color_ref("{ color = \"azul\", alpha = 0.5 }"),
            Ok(ColorRef {
                base: Base::Palette("azul".to_owned()),
                alpha: 0.5
            })
        );
    }

    #[test]
    fn valores_invalidos() {
        for (text, expected) in [
            ("\"#7aa2f\"", "no es un color"),
            ("\"#7aa2fg\"", "no es un color"),
            (
                "{ color = \"azul\", alpha = 2.0 }",
                "alpha debe estar entre 0 y 1",
            ),
            ("{ alpha = 0.5 }", "missing field `color`"),
            ("{ color = \"azul\", alfa = 0.5 }", "unknown field `alfa`"),
            ("3", "un color `#rrggbb`"),
        ] {
            let error = color_ref(text).unwrap_err();
            assert!(error.contains(expected), "{text}: {error}");
        }
    }

    #[test]
    fn debug_legible_para_los_snapshots() {
        let color = Color {
            rgb: 0x00_a2_f7,
            alpha: 1.,
        };
        assert_eq!(format!("{color:?}"), "#00a2f7");
        assert_eq!(
            format!(
                "{:?}",
                Color {
                    alpha: 0.3,
                    ..color
                }
            ),
            "#00a2f7 α0.3"
        );
    }
}
