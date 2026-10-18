//! Búsqueda en el buffer: el dialecto de vim, el recorrido con wrap y la
//! palabra de `*`.

mod pattern;
mod scan;
mod translate;
mod word;

pub(crate) use pattern::Case;
pub use pattern::Pattern;
pub(crate) use scan::Query;
pub use scan::Wrap;
pub(crate) use word::Word;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SearchError {
    #[error("E35: no hay un patrón anterior")]
    NoPrevious,
    #[error("E486: no se encontró el patrón: {0}")]
    NotFound(String),
    #[error("E348: no hay texto bajo el cursor")]
    NoStringUnderCursor,
    #[error("patrón inválido: {0}")]
    Invalid(String),
    #[error("nota no entiende esto del dialecto de vim: {0}")]
    Unsupported(String),
}

/// Separa lo escrito tras `/` (o el delimitador de `:s`) en el patrón y lo
/// que sigue al delimitador que lo cierra, si lo hay. Como vim, en una
/// búsqueda con `?` el `\?` es un `?` literal.
pub(crate) fn split_at_delimiter(text: &str, delimiter: char) -> (String, Option<&str>) {
    let mut pattern = String::new();
    let mut chars = text.char_indices();
    while let Some((at, c)) = chars.next() {
        if c == delimiter {
            return (pattern, Some(&text[at + c.len_utf8()..]));
        }
        if c != '\\' {
            pattern.push(c);
            continue;
        }
        match chars.next() {
            Some((_, '?')) if delimiter == '?' => pattern.push('?'),
            Some((_, escaped)) => {
                pattern.push('\\');
                pattern.push(escaped);
            }
            None => pattern.push('\\'),
        }
    }
    (pattern, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_the_first_unescaped_delimiter() {
        assert_eq!(split_at_delimiter("foo", '/'), ("foo".to_owned(), None));
        assert_eq!(
            split_at_delimiter("a\\/b/e", '/'),
            ("a\\/b".to_owned(), Some("e"))
        );
        assert_eq!(
            split_at_delimiter("a\\?b?", '?'),
            ("a?b".to_owned(), Some(""))
        );
        assert_eq!(split_at_delimiter("a\\", '/'), ("a\\".to_owned(), None));
    }
}
