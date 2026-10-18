//! Los números del margen de un panel, con las reglas de `number` y
//! `relativenumber` de vim.

/// Qué números muestra un panel. Como en vim, son de cada panel y un split
/// hereda los del panel que se divide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineNumbers {
    /// `number`: el número de cada línea.
    pub absolute: bool,
    /// `relativenumber`: la distancia a la línea del cursor. Con
    /// `absolute`, la del cursor muestra su número: el modo híbrido.
    pub relative: bool,
}

/// `numberwidth` de vim: columnas mínimas, contando el espacio que separa
/// el número del texto.
const MIN_WIDTH: usize = 4;

/// Lo que va en el margen de una línea, ya alineado al ancho del margen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub text: String,
    /// Es la línea del cursor, que se resalta.
    pub current: bool,
}

impl LineNumbers {
    #[cfg(test)]
    pub const HYBRID: Self = Self {
        absolute: true,
        relative: true,
    };

    #[must_use]
    pub fn shown(self) -> bool {
        self.absolute || self.relative
    }

    /// Columnas del margen, con el espacio final; 0 si no hay números. Como
    /// vim, con solo `relativenumber` el número más ancho posible es el de
    /// filas del panel y, si no, el de líneas del documento.
    #[must_use]
    pub fn width(self, line_count: usize, rows: usize) -> usize {
        if !self.shown() {
            return 0;
        }
        let widest = if self.absolute { line_count } else { rows };
        (digits(widest) + 1).max(MIN_WIDTH)
    }

    /// El número de `line` con el cursor en `cursor_line`, desde 0, en un
    /// margen de `width` columnas. Todos van a la derecha, también el de la
    /// línea del cursor en el modo híbrido, como en nvim.
    #[must_use]
    pub fn label(self, line: usize, cursor_line: usize, width: usize) -> Option<Label> {
        let digits = width.checked_sub(1)?;
        let current = line == cursor_line;
        let number = match (self.absolute, self.relative) {
            (false, false) => return None,
            (true, false) => line + 1,
            (true, true) if current => line + 1,
            (_, true) => line.abs_diff(cursor_line),
        };
        let text = format!("{number:>digits$} ");
        Some(Label { text, current })
    }
}

fn digits(mut n: usize) -> usize {
    let mut digits = 1;
    while n >= 10 {
        n /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const OFF: LineNumbers = LineNumbers {
        absolute: false,
        relative: false,
    };
    const NUMBER: LineNumbers = LineNumbers {
        absolute: true,
        relative: false,
    };
    const RELATIVE: LineNumbers = LineNumbers {
        absolute: false,
        relative: true,
    };

    #[test]
    fn el_ancho_crece_con_las_lineas_como_numberwidth() {
        let cases = [
            (OFF, 5_000, 40, 0),
            (NUMBER, 1, 40, 4),
            (NUMBER, 999, 40, 4),
            (NUMBER, 1_000, 40, 5),
            (NUMBER, 123_456, 40, 7),
            (LineNumbers::HYBRID, 99_999, 40, 6),
            (RELATIVE, 123_456, 40, 4),
            (RELATIVE, 5, 1_200, 5),
        ];
        for (numbers, line_count, rows, expected) in cases {
            assert_eq!(
                numbers.width(line_count, rows),
                expected,
                "{numbers:?} con {line_count} líneas y {rows} filas"
            );
        }
    }

    fn labels(numbers: LineNumbers, cursor_line: usize) -> Vec<String> {
        let width = numbers.width(12, 40);
        (7..12)
            .filter_map(|line| numbers.label(line, cursor_line, width))
            .map(|label| label.text)
            .collect()
    }

    #[test]
    fn cada_modo_numera_como_vim() {
        assert_eq!(labels(OFF, 9), Vec::<String>::new());
        assert_eq!(labels(NUMBER, 9), ["  8 ", "  9 ", " 10 ", " 11 ", " 12 "]);
        assert_eq!(
            labels(RELATIVE, 9),
            ["  2 ", "  1 ", "  0 ", "  1 ", "  2 "]
        );
        assert_eq!(
            labels(LineNumbers::HYBRID, 9),
            ["  2 ", "  1 ", " 10 ", "  1 ", "  2 "],
            "el número del cursor va a la derecha, en la misma columna"
        );
    }

    #[test]
    fn solo_la_linea_del_cursor_se_resalta() {
        for numbers in [NUMBER, RELATIVE, LineNumbers::HYBRID] {
            let current: Vec<bool> = (0..3)
                .filter_map(|line| numbers.label(line, 1, 4))
                .map(|label| label.current)
                .collect();
            assert_eq!(current, [false, true, false], "{numbers:?}");
        }
    }

    #[test]
    fn sin_margen_no_hay_numeros() {
        assert_eq!(NUMBER.label(0, 0, 0), None);
    }
}
