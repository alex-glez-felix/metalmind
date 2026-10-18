//! La sangría que dejan `>`, `<`, `:>` y `:<`, como `shift_line` y
//! `tabstop_fromto` de nvim.

use std::num::NonZeroUsize;

use crate::columns::Columns;

/// Con qué corren la sangría `>` y `<`: las columnas de un nivel
/// (`shiftwidth`) y si se escribe solo con espacios (`expandtab`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Indent {
    pub shiftwidth: NonZeroUsize,
    pub expandtab: bool,
}

impl Default for Indent {
    /// Lo de `nvim --clean`: `sw=8 noet`.
    fn default() -> Self {
        Self {
            shiftwidth: NonZeroUsize::MIN.saturating_add(7),
            expandtab: false,
        }
    }
}

impl Indent {
    /// Las columnas que corren `levels` niveles.
    pub(crate) fn levels_width(self, levels: usize) -> usize {
        levels.saturating_mul(self.shiftwidth.get())
    }

    /// Lo que llena de la columna `from` a la `to`: con `noexpandtab`, tabs
    /// mientras quepan y luego espacios (`tabstop_fromto` de nvim).
    pub(crate) fn whitespace(self, from: usize, to: usize, columns: Columns) -> String {
        let mut spaces = to.saturating_sub(from);
        if self.expandtab {
            return " ".repeat(spaces);
        }
        let tabstop = columns.tabstop();
        let mut tabs = 0;
        let to_next_stop = tabstop - from % tabstop;
        if spaces >= to_next_stop {
            spaces -= to_next_stop;
            tabs = 1 + spaces / tabstop;
            spaces %= tabstop;
        }
        format!("{}{}", "\t".repeat(tabs), " ".repeat(spaces))
    }
}

/// Hacia dónde corren la sangría `>` y `<`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shift {
    Right,
    Left,
}

/// Los blancos al inicio de `line`.
pub(crate) fn leading_blanks(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// La sangría de `line` corrida `width` columnas hacia `shift`, sin pasar
/// de la columna 0: cuántos bytes ocupa la de ahora y la nueva. `None` si
/// la línea está vacía, que vim no toca, o si la sangría no cambia.
pub(crate) fn shift(
    line: &str,
    shift: Shift,
    width: usize,
    columns: Columns,
    indent: Indent,
) -> Option<(usize, String)> {
    if line.is_empty() {
        return None;
    }
    let blanks = leading_blanks(line);
    let current = columns.line_width(blanks);
    let new_width = match shift {
        Shift::Right => current.saturating_add(width),
        Shift::Left => current.saturating_sub(width),
    };
    let new = indent.whitespace(0, new_width, columns);
    (new != blanks).then_some((blanks.len(), new))
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
        let indent = Indent::default();
        let width = indent.levels_width(levels.unsigned_abs());
        shift(line, direction, width, Columns::default(), indent)
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
        let indent = Indent::default();
        let (_, new) = shift("abc", Shift::Right, 8, columns, indent).unwrap();
        assert_eq!(new, "\t\t");
        let (_, new) = shift("\t\t\tabc", Shift::Left, 8, columns, indent).unwrap();
        assert_eq!(new, "\t");
    }

    #[test]
    fn expandtab_writes_only_spaces() {
        let indent = Indent {
            shiftwidth: NonZeroUsize::new(4).unwrap(),
            expandtab: true,
        };
        let columns = Columns::new(4);
        let (len, new) = shift("\t a", Shift::Right, 4, columns, indent).unwrap();
        assert_eq!((len, new.as_str()), (2, "         "));
        assert_eq!(indent.whitespace(2, 9, columns), "       ");
    }

    #[test]
    fn whitespace_starts_with_what_reaches_the_next_tabstop() {
        let columns = Columns::default();
        let indent = Indent::default();
        assert_eq!(indent.whitespace(2, 18, columns), "\t\t  ");
        assert_eq!(indent.whitespace(2, 7, columns), "     ");
        assert_eq!(indent.whitespace(2, 8, columns), "\t");
        assert_eq!(indent.whitespace(0, 16, columns), "\t\t");
    }
}
