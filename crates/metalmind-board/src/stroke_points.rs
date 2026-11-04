//! Los puntos de un trazo libre como texto compacto: décimas de píxel, cada
//! punto relativo al anterior, `dx,dy` o `dx,dy,presión` separados por
//! espacios. `0,0 12,-3 15,-1,180` se lee con un `split` en cualquier lenguaje.

use std::fmt::Write as _;

use thiserror::Error;

use crate::geometry::Point;

const TENTHS_PER_PX: f32 = 10.0;

/// En décimas enteras, las del archivo: un `f32` no las conservaría todas y
/// reescribir un trazo lejos del origen cambiaría sus bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrokePoint {
    x_tenths: i32,
    y_tenths: i32,
    /// 0–255; `None` con ratón.
    pub pressure: Option<u8>,
}

impl StrokePoint {
    /// El punto en `at`, relativo a la esquina del nodo.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "las coordenadas de una pizarra caben en i32"
    )]
    pub fn at(at: Point) -> Self {
        Self {
            x_tenths: (at.x * TENTHS_PER_PX).round() as i32,
            y_tenths: (at.y * TENTHS_PER_PX).round() as i32,
            pressure: None,
        }
    }

    /// Este punto al cambiar la caja de su trazo: a lo ancho `x_scale` y a lo
    /// alto `y_scale`.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "las coordenadas de una pizarra caben en i32 y en f32"
    )]
    pub(crate) fn scaled(self, x_scale: f32, y_scale: f32) -> Self {
        Self {
            x_tenths: (self.x_tenths as f32 * x_scale).round() as i32,
            y_tenths: (self.y_tenths as f32 * y_scale).round() as i32,
            ..self
        }
    }

    /// Relativo a la esquina del nodo.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "para pintar basta con la precisión de f32"
    )]
    pub fn position(&self) -> Point {
        Point::new(
            self.x_tenths as f32 / TENTHS_PER_PX,
            self.y_tenths as f32 / TENTHS_PER_PX,
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PointsError {
    #[error("punto {index}: `{text}` no es `dx,dy` ni `dx,dy,presión`")]
    Malformed { index: usize, text: String },
}

pub(crate) fn encode(points: &[StrokePoint]) -> String {
    let mut out = String::with_capacity(points.len() * 6);
    let (mut last_x, mut last_y) = (0, 0);
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        // Un `decode` ya comprobó que cada delta cabe: la resta no da la vuelta.
        let (dx, dy) = (
            point.x_tenths.wrapping_sub(last_x),
            point.y_tenths.wrapping_sub(last_y),
        );
        let _ = write!(out, "{dx},{dy}");
        if let Some(pressure) = point.pressure {
            let _ = write!(out, ",{pressure}");
        }
        (last_x, last_y) = (point.x_tenths, point.y_tenths);
    }
    out
}

pub(crate) fn decode(text: &str) -> Result<Vec<StrokePoint>, PointsError> {
    let (mut x, mut y) = (0_i32, 0_i32);
    text.split_ascii_whitespace()
        .enumerate()
        .map(|(index, chunk)| {
            let malformed = || PointsError::Malformed {
                index,
                text: chunk.to_owned(),
            };
            let mut parts = chunk.split(',');
            let mut delta = || parts.next().and_then(|p| p.parse::<i32>().ok());
            let dx = delta().ok_or_else(malformed)?;
            let dy = delta().ok_or_else(malformed)?;
            let pressure = parts
                .next()
                .map(|p| p.parse::<u8>().map_err(|_| malformed()))
                .transpose()?;
            if parts.next().is_some() {
                return Err(malformed());
            }
            x = x.checked_add(dx).ok_or_else(malformed)?;
            y = y.checked_add(dy).ok_or_else(malformed)?;
            Ok(StrokePoint {
                x_tenths: x,
                y_tenths: y,
                pressure,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("", "")]
    #[case("0,0 12,-3 15,-1,180", "0,0 12,-3 15,-1,180")]
    #[case("  0,0\n\t+5,7 ", "0,0 5,7")]
    #[case("2000000000,0 7,0", "2000000000,0 7,0")]
    fn encode_writes_what_decode_read(#[case] text: &str, #[case] canonical: &str) {
        assert_eq!(encode(&decode(text).unwrap()), canonical);
    }

    #[test]
    fn position_is_in_pixels_and_accumulates_the_deltas() {
        let points = decode("0,0 12,-3 15,-1,180").unwrap();
        assert_eq!(points[2].position(), Point::new(2.7, -0.4));
        assert_eq!(points[2].pressure, Some(180));
    }

    #[rstest]
    #[case("0,0 1", 1, "1")]
    #[case("0,0,300", 0, "0,0,300")]
    #[case("1,2,3,4", 0, "1,2,3,4")]
    #[case("a,b", 0, "a,b")]
    #[case("2147483647,0 1,0", 1, "1,0")]
    fn rejects_what_is_not_a_point(#[case] text: &str, #[case] index: usize, #[case] chunk: &str) {
        assert_eq!(
            decode(text),
            Err(PointsError::Malformed {
                index,
                text: chunk.to_owned()
            })
        );
    }
}
