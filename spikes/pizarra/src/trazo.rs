//! Los puntos de un trazo libre como texto compacto: décimas de píxel,
//! cada punto relativo al anterior, `dx,dy` o `dx,dy,presión` separados por
//! espacios. `0,0 12,-3 15,-1,180` se lee con un `split` en cualquier lenguaje.

use std::fmt::Write as _;

use thiserror::Error;

/// Décimas de píxel: lo que distingue un lápiz en una tableta con zoom.
const UNITS_PER_PX: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    /// 0–255; `None` con ratón.
    pub pressure: Option<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PointsError {
    #[error("punto {index}: `{text}` no es `dx,dy` ni `dx,dy,presión`")]
    Malformed { index: usize, text: String },
}

#[must_use]
pub fn encode(points: &[Point]) -> String {
    let mut out = String::with_capacity(points.len() * 6);
    let (mut last_x, mut last_y) = (0_i32, 0_i32);
    for (index, point) in points.iter().enumerate() {
        let (x, y) = (units(point.x), units(point.y));
        if index > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{},{}", x - last_x, y - last_y);
        if let Some(pressure) = point.pressure {
            let _ = write!(out, ",{pressure}");
        }
        (last_x, last_y) = (x, y);
    }
    out
}

/// # Errors
///
/// Un punto que no es `dx,dy` ni `dx,dy,presión` con enteros.
pub fn decode(text: &str) -> Result<Vec<Point>, PointsError> {
    let (mut x, mut y) = (0_i32, 0_i32);
    text.split_ascii_whitespace()
        .enumerate()
        .map(|(index, chunk)| {
            let malformed = || PointsError::Malformed {
                index,
                text: chunk.to_owned(),
            };
            let mut parts = chunk.split(',');
            let dx: i32 = parts
                .next()
                .and_then(|p| p.parse().ok())
                .ok_or_else(malformed)?;
            let dy: i32 = parts
                .next()
                .and_then(|p| p.parse().ok())
                .ok_or_else(malformed)?;
            let pressure = match parts.next() {
                Some(p) => Some(p.parse::<u8>().map_err(|_| malformed())?),
                None => None,
            };
            if parts.next().is_some() {
                return Err(malformed());
            }
            x = x.checked_add(dx).ok_or_else(malformed)?;
            y = y.checked_add(dy).ok_or_else(malformed)?;
            Ok(Point {
                x: px(x),
                y: px(y),
                pressure,
            })
        })
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "un trazo mide menos de 2^31 décimas de píxel"
)]
fn units(value: f32) -> i32 {
    (value * UNITS_PER_PX).round() as i32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "las décimas de un trazo caben de sobra en f32"
)]
fn px(units: i32) -> f32 {
    units as f32 / UNITS_PER_PX
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32, pressure: Option<u8>) -> Point {
        Point { x, y, pressure }
    }

    #[test]
    fn encodes_deltas_in_tenths_of_a_pixel() {
        let points = [
            point(0.0, 0.0, None),
            point(1.2, -0.3, None),
            point(2.7, -0.4, Some(180)),
        ];
        assert_eq!(encode(&points), "0,0 12,-3 15,-1,180");
    }

    #[test]
    fn decode_undoes_encode_to_a_tenth() {
        let points: Vec<Point> = (0..200)
            .map(|i| {
                #[expect(clippy::cast_precision_loss, reason = "pocos puntos")]
                let t = i as f32 * 0.37;
                point(
                    t * 13.1 - 400.0,
                    t.sin() * 77.7,
                    (i % 3 == 0).then_some(u8::try_from(i % 256).unwrap_or(0)),
                )
            })
            .collect();
        let back = decode(&encode(&points)).unwrap();
        assert_eq!(back.len(), points.len());
        for (a, b) in points.iter().zip(&back) {
            assert!(
                (a.x - b.x).abs() <= 0.051 && (a.y - b.y).abs() <= 0.051,
                "{a:?} {b:?}"
            );
            assert_eq!(a.pressure, b.pressure);
        }
        // Lo decodificado ya está en décimas: volver a codificar no cambia nada.
        assert_eq!(encode(&back), encode(&points));
    }

    #[test]
    fn rejects_what_is_not_a_point() {
        assert_eq!(
            decode("0,0 1"),
            Err(PointsError::Malformed {
                index: 1,
                text: "1".to_owned()
            })
        );
        assert!(decode("0,0,300").is_err(), "la presión es un byte");
        assert!(decode("1,2,3,4").is_err());
        assert_eq!(
            decode("2147483647,0 1,0"),
            Err(PointsError::Malformed {
                index: 1,
                text: "1,0".to_owned()
            }),
            "la suma de los deltas se desborda"
        );
        assert_eq!(decode(""), Ok(Vec::new()));
    }
}
