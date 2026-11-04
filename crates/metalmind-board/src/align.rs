//! Dónde cae lo que se arrastra: en la rejilla o, si un vecino queda cerca, en
//! línea con su borde o su centro, con la guía que lo muestra (09 §4.4).

use crate::edit::snap;
use crate::geometry::Rect;

/// Una línea de alineación entre lo que se mueve y sus vecinos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// `x` constante, de `from` a `to` en `y`.
    Vertical { x: f32, from: f32, to: f32 },
    /// `y` constante, de `from` a `to` en `x`.
    Horizontal { y: f32, from: f32, to: f32 },
}

/// A partir de esta distancia dos líneas ya no son la misma: un centro puede
/// caer en medio píxel y el archivo solo guarda enteros.
const SAME_LINE: f32 = 0.51;

#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub guides: Vec<Guide>,
}

/// Las tres líneas de un rectángulo en un eje.
fn lines(start: f32, length: f32) -> [f32; 3] {
    [start, start + length / 2.0, start + length]
}

/// Lo que hay que correr `start` para alinearlo con el vecino más cercano a
/// menos de `reach`.
fn nearest_shift(start: f32, length: f32, others: &[(f32, f32)], reach: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for line in lines(start, length) {
        for &(other_start, other_length) in others {
            for other in lines(other_start, other_length) {
                let shift = other - line;
                if shift.abs() < reach && best.is_none_or(|found| shift.abs() < found.abs()) {
                    best = Some(shift);
                }
            }
        }
    }
    best
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "las coordenadas de una pizarra caben en i32"
)]
fn settled(start: f32, length: f32, others: &[(f32, f32)], reach: f32) -> i32 {
    nearest_shift(start, length, others, reach)
        .map_or_else(|| snap(start), |shift| (start + shift).round() as i32)
}

/// Lleva `moving`, que está donde lo deja el ratón, a su lugar: cada eje se
/// alinea con un vecino a menos de `reach` unidades o, si no hay, va a la rejilla.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "las coordenadas de una pizarra caben en f32"
)]
pub fn place(moving: Rect, neighbors: &[Rect], reach: f32) -> Placement {
    let across: Vec<(f32, f32)> = neighbors.iter().map(|n| (n.x, n.w)).collect();
    let down: Vec<(f32, f32)> = neighbors.iter().map(|n| (n.y, n.h)).collect();
    let x = settled(moving.x, moving.w, &across, reach);
    let y = settled(moving.y, moving.h, &down, reach);
    let at = Rect {
        x: x as f32,
        y: y as f32,
        ..moving
    };
    Placement {
        x,
        y,
        guides: guides(at, neighbors),
    }
}

/// Una guía por cada línea de `at` que coincide con alguna de un vecino, tan
/// larga como lo que une.
fn guides(at: Rect, neighbors: &[Rect]) -> Vec<Guide> {
    let mut found = Vec::new();
    for line in lines(at.x, at.w) {
        let touching = neighbors
            .iter()
            .filter(|n| lines(n.x, n.w).iter().any(|o| (o - line).abs() < SAME_LINE));
        if let Some((from, to)) = span(at.y, at.h, touching.map(|n| (n.y, n.h))) {
            found.push(Guide::Vertical { x: line, from, to });
        }
    }
    for line in lines(at.y, at.h) {
        let touching = neighbors
            .iter()
            .filter(|n| lines(n.y, n.h).iter().any(|o| (o - line).abs() < SAME_LINE));
        if let Some((from, to)) = span(at.x, at.w, touching.map(|n| (n.x, n.w))) {
            found.push(Guide::Horizontal { y: line, from, to });
        }
    }
    found
}

/// De dónde a dónde llega lo que se mueve junto con sus vecinos, si tiene alguno.
fn span(start: f32, length: f32, others: impl Iterator<Item = (f32, f32)>) -> Option<(f32, f32)> {
    let mut any = false;
    let (mut from, mut to) = (start, start + length);
    for (other_start, other_length) in others {
        any = true;
        from = from.min(other_start);
        to = to.max(other_start + other_length);
    }
    any.then_some((from, to))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    const fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn sin_vecinos_va_a_la_rejilla() {
        let placed = place(rect(14.0, 26.0, 100.0, 50.0), &[], 6.0);
        assert_eq!(
            placed,
            Placement {
                x: 10,
                y: 30,
                guides: Vec::new()
            }
        );
    }

    #[rstest]
    // Bordes izquierdos.
    #[case(rect(304.0, 900.0, 100.0, 50.0), 300)]
    // Derecho con izquierdo.
    #[case(rect(197.0, 900.0, 100.0, 50.0), 200)]
    // Centros: el de 100 en 350 y el de 200 en 400.
    #[case(rect(352.0, 900.0, 100.0, 50.0), 350)]
    fn se_alinea_con_borde_o_centro_de_un_vecino(#[case] moving: Rect, #[case] x: i32) {
        let vecino = rect(300.0, 0.0, 200.0, 50.0);
        let placed = place(moving, &[vecino], 6.0);
        assert_eq!(placed.x, x);
        assert!(placed.guides.iter().any(|guide| matches!(
            guide,
            Guide::Vertical { from, to, .. } if *from == 0.0 && *to > 900.0
        )));
    }

    #[test]
    fn un_vecino_fuera_de_alcance_no_atrae() {
        let placed = place(
            rect(308.0, 900.0, 100.0, 50.0),
            &[rect(300.0, 0.0, 200.0, 50.0)],
            6.0,
        );
        assert_eq!(placed.x, 310);
        assert_eq!(placed.guides, Vec::new());
    }

    #[test]
    fn gana_el_mas_cercano_y_los_ejes_son_independientes() {
        let neighbors = [rect(305.0, 0.0, 10.0, 10.0), rect(0.0, 503.0, 10.0, 10.0)];
        let placed = place(rect(301.0, 500.0, 100.0, 50.0), &neighbors, 6.0);
        assert_eq!((placed.x, placed.y), (305, 503));
    }
}
