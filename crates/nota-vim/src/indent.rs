//! La sangría que dejan `>`, `<`, `:>` y `:<`, como `shift_line` y
//! `tabstop_fromto` de nvim con `noexpandtab`.

use crate::columns::Columns;

/// El `shiftwidth` de `nvim --clean`: cada nivel son 8 columnas. Saldrá de
/// las opciones cuando `:set` llegue al motor.
pub(crate) const SHIFTWIDTH: usize = 8;

/// Hacia dónde corren la sangría `>` y `<`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shift {
    Right,
    Left,
}

/// Las columnas que corren `levels` niveles.
pub(crate) fn levels_width(levels: usize) -> usize {
    levels.saturating_mul(SHIFTWIDTH)
}

/// La sangría de `line` corrida `levels` niveles hacia `shift`, sin pasar
/// de la columna 0: cuántos bytes ocupa la de ahora y la nueva, con tabs y
/// luego espacios. `None` si la línea está vacía, que vim no toca, o si la
/// sangría no cambia.
pub(crate) fn shift(
    line: &str,
    shift: Shift,
    levels: usize,
    columns: Columns,
) -> Option<(usize, String)> {
    if line.is_empty() {
        return None;
    }
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let indent = &line[..indent_len];
    let width = columns.line_width(indent);
    let new_width = match shift {
        Shift::Right => width.saturating_add(levels_width(levels)),
        Shift::Left => width.saturating_sub(levels_width(levels)),
    };
    let new = whitespace(0, new_width, columns);
    (new != indent).then_some((indent_len, new))
}

/// Lo que llena de la columna `from` a la `to`: tabs mientras quepan y
/// luego espacios (`tabstop_fromto` de nvim).
pub(crate) fn whitespace(from: usize, to: usize, columns: Columns) -> String {
    let tabstop = columns.tabstop();
    let mut spaces = to.saturating_sub(from);
    let mut tabs = 0;
    let to_next_stop = tabstop - from % tabstop;
    if spaces >= to_next_stop {
        spaces -= to_next_stop;
        tabs = 1 + spaces / tabstop;
        spaces %= tabstop;
    }
    format!("{}{}", "\t".repeat(tabs), " ".repeat(spaces))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shifted(line: &str, levels: isize) -> Option<String> {
        let direction = if levels < 0 {
            Shift::Left
        } else {
            Shift::Right
        };
        shift(line, direction, levels.unsigned_abs(), Columns::default())
            .map(|(len, new)| format!("{new}{}", &line[len..]))
    }

    #[test]
    fn shifts_like_vim() {
        let cases = [
            ("abc", 1, Some("\tabc")),
            ("    abc", 1, Some("\t    abc")),
            ("abc", 3, Some("\t\t\tabc")),
            ("   ", 1, Some("\t   ")),
            ("\tabc", -1, Some("abc")),
            ("\t  abc", -1, Some("  abc")),
            ("           abc", -1, Some("   abc")),
            ("  abc", -1, Some("abc")),
            ("abc", -1, None),
            ("", 1, None),
        ];
        for (line, levels, expected) in cases {
            assert_eq!(shifted(line, levels).as_deref(), expected, "{line:?}");
        }
    }

    #[test]
    fn a_tabstop_of_four_puts_two_tabs_in_a_level() {
        let columns = Columns::new(4);
        let (_, new) = shift("abc", Shift::Right, 1, columns).unwrap();
        assert_eq!(new, "\t\t");
        let (_, new) = shift("\t\t\tabc", Shift::Left, 1, columns).unwrap();
        assert_eq!(new, "\t");
    }

    #[test]
    fn whitespace_starts_with_what_reaches_the_next_tabstop() {
        let columns = Columns::default();
        assert_eq!(whitespace(2, 18, columns), "\t\t  ");
        assert_eq!(whitespace(2, 7, columns), "     ");
        assert_eq!(whitespace(2, 8, columns), "\t");
        assert_eq!(whitespace(0, 16, columns), "\t\t");
    }
}
