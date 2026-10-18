//! La sangría que dejan `:>` y `:<`.

/// Los de `nvim --clean`: un nivel es un tab. Saldrán de las opciones
/// cuando `:set` llegue al motor.
const SHIFTWIDTH: usize = 8;
const TABSTOP: usize = 8;

/// La sangría de `line` corrida `levels` niveles (a la izquierda si es
/// negativo), sin pasar de la columna 0: cuántos bytes ocupa la de ahora y
/// la nueva, con tabs y luego espacios (`noexpandtab`). `None` si la línea
/// está vacía, que vim no toca, o si la sangría no cambia.
pub(crate) fn shift(line: &str, levels: isize) -> Option<(usize, String)> {
    if line.is_empty() {
        return None;
    }
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let indent = &line[..indent_len];
    let width = indent.chars().fold(0, |width, c| match c {
        '\t' => width + TABSTOP - width % TABSTOP,
        _ => width + 1,
    });
    let delta = levels.saturating_mul(SHIFTWIDTH.cast_signed());
    let new_width = width.saturating_add_signed(delta);
    let new = format!(
        "{}{}",
        "\t".repeat(new_width / TABSTOP),
        " ".repeat(new_width % TABSTOP)
    );
    (new != indent).then_some((indent_len, new))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shifted(line: &str, levels: isize) -> Option<String> {
        shift(line, levels).map(|(len, new)| format!("{new}{}", &line[len..]))
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
}
