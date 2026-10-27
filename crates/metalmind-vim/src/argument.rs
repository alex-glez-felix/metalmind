//! Las palabras del argumento de un comando de `:`, como `:focus Mis\ notas/
//! #rust`. Un espacio separa palabras salvo escapado con `\`, como en vim, o
//! entre comillas dobles. `\` escapa cualquier carácter, también `\` y `"`.

use std::ops::Range;

/// Una palabra del argumento, con lo que significa ya sin escapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgumentWord {
    /// En bytes de lo escrito, con sus escapes y comillas.
    pub range: Range<usize>,
    pub text: String,
    /// Si su primer carácter iba escapado o entre comillas: `\#ideas` es
    /// el texto `#ideas`, no un tag.
    pub literal_start: bool,
}

/// Las palabras de `argument`, en orden. Una comilla sin cerrar llega al
/// final, y un `\` al final es él mismo.
#[must_use]
pub fn argument_words(argument: &str) -> Vec<ArgumentWord> {
    let mut words = Vec::new();
    let mut current: Option<ArgumentWord> = None;
    let mut quoted = false;
    let mut chars = argument.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c.is_whitespace() && !quoted {
            words.extend(current.take());
            continue;
        }
        let word = current.get_or_insert_with(|| ArgumentWord {
            range: at..at,
            text: String::new(),
            literal_start: false,
        });
        match c {
            '"' => quoted = !quoted,
            '\\' => word.push_literal(chars.next().map_or('\\', |(_, escaped)| escaped)),
            _ if quoted => word.push_literal(c),
            _ => word.text.push(c),
        }
        word.range.end = chars.peek().map_or(argument.len(), |&(next, _)| next);
    }
    words.extend(current);
    words
}

impl ArgumentWord {
    fn push_literal(&mut self, c: char) {
        self.literal_start |= self.text.is_empty();
        self.text.push(c);
    }
}

/// `text` escrito para que [`argument_words`] lo lea como una sola palabra
/// igual: escapa los espacios, `\` y `"`.
#[must_use]
pub fn escape_argument(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_whitespace() || c == '\\' || c == '"' {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    use super::*;

    fn texts(argument: &str) -> Vec<(String, bool)> {
        argument_words(argument)
            .into_iter()
            .map(|word| (word.text, word.literal_start))
            .collect()
    }

    #[rstest::rstest]
    #[case::espacios("  a  b ", &[("a", false), ("b", false)])]
    #[case::escape_como_vim(r"Mis\ notas/ #rust", &[("Mis notas/", false), ("#rust", false)])]
    #[case::comillas(r#""Mis notas"/ -#"#, &[("Mis notas/", true), ("-#", false)])]
    #[case::comillas_en_medio(r#"Mis" "notas"#, &[("Mis notas", false)])]
    #[case::comilla_sin_cerrar(r#""Mis no"#, &[("Mis no", true)])]
    #[case::comillas_vacias(r#""""#, &[("", false)])]
    #[case::escape_al_principio(r"\#ideas \-#x", &[("#ideas", true), ("-#x", true)])]
    #[case::escapes_de_escape(r#"a\\b \"c"#, &[(r"a\b", false), ("\"c", true)])]
    #[case::barra_al_final(r"a\", &[(r"a\", false)])]
    #[case::tab_escapado("a\\\tb", &[("a\tb", false)])]
    fn separa_como_vim(#[case] argument: &str, #[case] expected: &[(&str, bool)]) {
        let expected: Vec<(String, bool)> = expected
            .iter()
            .map(|&(text, literal)| (text.to_owned(), literal))
            .collect();
        assert_eq!(texts(argument), expected);
    }

    #[test]
    fn cada_palabra_sabe_donde_se_escribio() {
        let ranges: Vec<Range<usize>> = argument_words(r#" a\ b  "c d"x e "#)
            .into_iter()
            .map(|word| word.range)
            .collect();
        assert_eq!(ranges, [1..5, 7..13, 14..15]);
    }

    proptest! {
        #[test]
        fn escapar_se_lee_igual(text in ".+") {
            let words = argument_words(&escape_argument(&text));
            prop_assert_eq!(words.len(), 1);
            prop_assert_eq!(&words[0].text, &text);
        }
    }
}
