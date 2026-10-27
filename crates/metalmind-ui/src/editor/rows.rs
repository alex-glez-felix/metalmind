//! Dónde va cada línea cuando no todas miden lo mismo (un heading es más
//! alto), sin nada de GPUI: la vista pinta las líneas donde dice esto y la
//! rueda avanza por lo que mide cada una.

use super::scroll::{Scroll, to_f64};

/// Una línea visible: su número, dónde empieza y cuánto mide, en píxeles
/// desde arriba del área de texto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub line: usize,
    pub top: f32,
    pub height: f32,
}

/// Las líneas que se ven desde `scroll` en un área de `height` píxeles: la
/// primera puede asomar recortada arriba y la última abajo.
pub fn place(
    scroll: Scroll,
    last_line: usize,
    height: f32,
    line_height: impl Fn(usize) -> f32,
) -> Vec<Placed> {
    let first = scroll.first_line().min(last_line);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "la fracción está en [0, 1) y una línea mide pocos píxeles"
    )]
    let hidden = (scroll.fraction() * f64::from(line_height(first))) as f32;
    let mut top = -hidden;
    let mut placed = Vec::new();
    for line in first..=last_line {
        if top >= height {
            break;
        }
        let tall = line_height(line);
        placed.push(Placed {
            line,
            top,
            height: tall,
        });
        top += tall;
    }
    placed
}

/// `scroll` movido `delta` píxeles (hacia abajo si es positivo): la
/// fracción es de la línea de arriba, así que un heading alto se recorre
/// más despacio en líneas que el texto. La última línea puede ocultar sus
/// filas de arriba (una partida más alta que la ventana) hasta dejar ver
/// una fila de `row` píxeles.
pub fn scroll_by_pixels(
    scroll: Scroll,
    delta: f64,
    last_line: usize,
    row: f64,
    line_height: impl Fn(usize) -> f64,
) -> Scroll {
    let mut line = scroll.first_line().min(last_line);
    let mut offset = scroll.fraction() * line_height(line) + delta;
    while offset < 0.0 && line > 0 {
        line -= 1;
        offset += line_height(line);
    }
    while offset >= line_height(line) && line < last_line {
        offset -= line_height(line);
        line += 1;
    }
    let height = line_height(line);
    let most = if line == last_line {
        ((height - row) / height).max(0.0)
    } else {
        1.0 - f64::EPSILON
    };
    let fraction = (offset / height).clamp(0.0, most);
    let top = if offset < 0.0 {
        0.0
    } else {
        to_f64(line) + fraction
    };
    Scroll::at(top)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// El texto mide 20 y la línea 2, un heading, 50.
    fn heights(line: usize) -> f32 {
        if line == 2 { 50.0 } else { 20.0 }
    }

    fn tops(placed: &[Placed]) -> Vec<(usize, f32, f32)> {
        placed
            .iter()
            .map(|line| (line.line, line.top, line.height))
            .collect()
    }

    #[test]
    fn a_tall_line_pushes_the_ones_below() {
        let placed = place(Scroll::at(0.0), 10, 100.0, heights);
        assert_eq!(
            tops(&placed),
            [
                (0, 0.0, 20.0),
                (1, 20.0, 20.0),
                (2, 40.0, 50.0),
                (3, 90.0, 20.0)
            ]
        );
    }

    #[test]
    fn the_top_line_peeks_by_its_own_height() {
        let placed = place(Scroll::at(2.5), 10, 60.0, heights);
        assert_eq!(
            tops(&placed),
            [(2, -25.0, 50.0), (3, 25.0, 20.0), (4, 45.0, 20.0)]
        );
    }

    #[test]
    fn the_wheel_crosses_a_tall_line_by_its_pixels() {
        let height = |line| f64::from(heights(line));
        let down = scroll_by_pixels(Scroll::at(0.0), 65.0, 10, 20.0, height);
        assert_eq!((down.first_line(), down.fraction()), (2, 0.5));
        let back = scroll_by_pixels(down, -65.0, 10, 20.0, height);
        assert_eq!((back.first_line(), back.fraction()), (0, 0.0));
        let past_end = scroll_by_pixels(Scroll::at(9.0), 500.0, 10, 20.0, height);
        assert_eq!(past_end, Scroll::at(10.0));
        let before_start = scroll_by_pixels(Scroll::at(1.0), -500.0, 10, 20.0, height);
        assert_eq!(before_start, Scroll::at(0.0));
        let tall_last = |line| if line == 10 { 100.0 } else { 20.0 };
        let hidden = scroll_by_pixels(Scroll::at(10.0), 50.0, 10, 20.0, tall_last);
        assert_eq!(hidden, Scroll::at(10.5), "una última línea alta se recorre");
        let past = scroll_by_pixels(Scroll::at(10.5), 500.0, 10, 20.0, tall_last);
        assert_eq!(past, Scroll::at(10.8), "hasta dejar ver su última fila");
    }
}
