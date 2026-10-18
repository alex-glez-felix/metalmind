//! El texto de reemplazo de `:s`, con `&`, `\0`…`\9` y saltos de línea.

use regex::Captures;

use crate::CommandError;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    /// `&` o `\0` es toda la coincidencia; `\1`…`\9`, cada grupo.
    Group(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Replacement(Vec<Piece>);

impl Replacement {
    /// Como vim, `\r` parte la línea; a diferencia de vim, `\n` también (vim
    /// mete un carácter NUL, que en una nota no sirve). `~` es texto.
    ///
    /// # Errors
    /// [`CommandError::UnsupportedReplacement`] con `\u`, `\U`, `\l`, `\L`,
    /// `\e` o `\E`.
    pub fn parse(text: &str) -> Result<Self, CommandError> {
        let mut pieces = Vec::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            let piece = match c {
                '&' => Piece::Group(0),
                '\\' => match chars.next() {
                    Some(digit @ '0'..='9') => Piece::Group(digit as usize - '0' as usize),
                    Some('r' | 'n') => Piece::Text("\n".to_owned()),
                    Some('t') => Piece::Text("\t".to_owned()),
                    Some(c @ ('u' | 'U' | 'l' | 'L' | 'e' | 'E')) => {
                        return Err(CommandError::UnsupportedReplacement(format!("\\{c}")));
                    }
                    Some(c) => Piece::Text(c.to_string()),
                    None => Piece::Text("\\".to_owned()),
                },
                c => Piece::Text(c.to_string()),
            };
            match (pieces.last_mut(), piece) {
                (Some(Piece::Text(last)), Piece::Text(text)) => last.push_str(&text),
                (_, piece) => pieces.push(piece),
            }
        }
        Ok(Self(pieces))
    }

    pub fn expand(&self, captures: &Captures<'_>) -> String {
        let mut expanded = String::new();
        for piece in &self.0 {
            match piece {
                Piece::Text(text) => expanded.push_str(text),
                Piece::Group(group) => {
                    if let Some(found) = captures.get(*group) {
                        expanded.push_str(found.as_str());
                    }
                }
            }
        }
        expanded
    }
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use super::*;

    fn expand(replacement: &str, regex: &str, text: &str) -> String {
        let captures = Regex::new(regex).unwrap().captures(text).unwrap();
        Replacement::parse(replacement).unwrap().expand(&captures)
    }

    #[test]
    fn expands_like_vim() {
        assert_eq!(expand("x", "b", "abc"), "x");
        assert_eq!(expand("[&]", "b", "abc"), "[b]");
        assert_eq!(expand("\\&\\\\", "b", "abc"), "&\\");
        assert_eq!(expand("\\2-\\1-\\0", "(a)(b)", "ab"), "b-a-ab");
        assert_eq!(expand("\\3", "(a)", "a"), "");
        assert_eq!(expand("1\\r2\\n3\\t", "b", "b"), "1\n2\n3\t");
        assert_eq!(expand("\\/\\.~", "b", "b"), "/.~");
        assert_eq!(expand("a\\", "b", "b"), "a\\");
    }

    #[test]
    fn rejects_case_modifiers() {
        assert_eq!(
            Replacement::parse("\\u&"),
            Err(CommandError::UnsupportedReplacement("\\u".to_owned()))
        );
    }
}
