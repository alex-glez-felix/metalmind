//! Las líneas de un bloque de código cercado y un resaltado barato para
//! los lenguajes más comunes: palabras clave, textos, números y comentarios
//! de una línea. Cada línea se mira sola, así que un texto o un comentario
//! de varias líneas solo se pinta en la primera.

use std::ops::Range;

use super::{Role, Token};
use crate::code::Fence;

/// La cerca que abrió el bloque y el lenguaje de su info string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenFence {
    pub fence: Fence,
    pub lang: Option<Lang>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lang {
    Rust,
    Python,
    Script,
    Shell,
    Go,
    CLike,
    Json,
    Yaml,
    Toml,
    Sql,
    Lua,
}

impl Lang {
    pub(crate) fn of(info: &str) -> Option<Self> {
        let name = info.split_whitespace().next()?.to_ascii_lowercase();
        Some(match name.as_str() {
            "rust" | "rs" => Self::Rust,
            "python" | "py" => Self::Python,
            "js" | "javascript" | "jsx" | "ts" | "typescript" | "tsx" | "mjs" => Self::Script,
            "sh" | "bash" | "zsh" | "shell" | "fish" | "console" => Self::Shell,
            "go" => Self::Go,
            "c" | "h" | "cpp" | "c++" | "hpp" | "java" | "kotlin" | "kt" | "swift" | "cs"
            | "csharp" => Self::CLike,
            "json" | "jsonc" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "toml" => Self::Toml,
            "sql" => Self::Sql,
            "lua" => Self::Lua,
            _ => return None,
        })
    }

    /// Las palabras clave, ordenadas para buscarlas en log n.
    fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
                "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super",
                "trait", "true", "type", "unsafe", "use", "where", "while",
            ],
            Self::Python => &[
                "False", "None", "True", "and", "as", "async", "await", "break", "class",
                "continue", "def", "elif", "else", "except", "finally", "for", "from", "if",
                "import", "in", "is", "lambda", "not", "or", "pass", "raise", "return", "try",
                "while", "with", "yield",
            ],
            Self::Script => &[
                "async",
                "await",
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "default",
                "else",
                "export",
                "extends",
                "false",
                "for",
                "from",
                "function",
                "if",
                "import",
                "in",
                "interface",
                "let",
                "new",
                "null",
                "of",
                "return",
                "switch",
                "this",
                "throw",
                "true",
                "try",
                "type",
                "typeof",
                "undefined",
                "var",
                "while",
            ],
            Self::Shell => &[
                "case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function",
                "if", "in", "local", "return", "then", "while",
            ],
            Self::Go => &[
                "break",
                "case",
                "chan",
                "const",
                "continue",
                "default",
                "defer",
                "else",
                "false",
                "for",
                "func",
                "go",
                "if",
                "import",
                "interface",
                "map",
                "nil",
                "package",
                "range",
                "return",
                "select",
                "struct",
                "switch",
                "true",
                "type",
                "var",
            ],
            Self::CLike => &[
                "break", "case", "class", "const", "continue", "default", "else", "enum", "false",
                "for", "if", "import", "new", "null", "private", "public", "return", "static",
                "struct", "switch", "this", "true", "void", "while",
            ],
            Self::Json | Self::Yaml | Self::Toml => &["false", "null", "true"],
            Self::Sql => &[
                "and", "by", "create", "delete", "from", "group", "insert", "into", "join", "not",
                "null", "or", "order", "select", "set", "table", "update", "values", "where",
            ],
            Self::Lua => &[
                "and", "do", "else", "elseif", "end", "false", "for", "function", "if", "in",
                "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
            ],
        }
    }

    fn line_comment(self) -> &'static [&'static str] {
        match self {
            Self::Rust | Self::Script | Self::Go | Self::CLike | Self::Json => &["//"],
            Self::Python | Self::Shell | Self::Yaml | Self::Toml => &["#"],
            Self::Sql | Self::Lua => &["--"],
        }
    }

    fn case_insensitive(self) -> bool {
        self == Self::Sql
    }
}

/// Los tokens de una línea de código, en bytes desde su inicio.
pub(crate) fn highlight(line: &str, lang: Lang) -> Vec<(Range<usize>, Role)> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let b = bytes[at];
        if lang
            .line_comment()
            .iter()
            .any(|c| line[at..].starts_with(c))
            && (lang != Lang::Shell || at == 0 || bytes[at - 1].is_ascii_whitespace())
        {
            out.push((at..line.len(), Role::Token(Token::Comment)));
            break;
        }
        if matches!(b, b'"' | b'\'' | b'`') {
            if let Some(end) = string_end(bytes, at) {
                out.push((at..end, Role::Token(Token::String)));
                at = end;
                continue;
            }
        } else if b.is_ascii_digit() && (at == 0 || !is_word(bytes[at - 1])) {
            let end = at
                + bytes[at..]
                    .iter()
                    .take_while(|&&b| is_word(b) || b == b'.')
                    .count();
            out.push((at..end, Role::Token(Token::Number)));
            at = end;
            continue;
        } else if is_word(b) {
            let end = at + bytes[at..].iter().take_while(|&&b| is_word(b)).count();
            if is_keyword(lang, &line[at..end]) {
                out.push((at..end, Role::Token(Token::Keyword)));
            }
            at = end;
            continue;
        }
        at += line[at..].chars().next().map_or(1, char::len_utf8);
    }
    out
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_keyword(lang: Lang, word: &str) -> bool {
    let keywords = lang.keywords();
    if lang.case_insensitive() {
        keywords
            .binary_search(&word.to_ascii_lowercase().as_str())
            .is_ok()
    } else {
        keywords.binary_search(&word).is_ok()
    }
}

/// El fin del texto que abre la comilla en `at`, si cierra en la línea.
fn string_end(bytes: &[u8], at: usize) -> Option<usize> {
    let quote = bytes[at];
    let mut i = at + 1;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'\\' => i += 2,
            _ if b == quote => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn tokens(line: &str, lang: Lang) -> Vec<(&str, Token)> {
        highlight(line, lang)
            .into_iter()
            .filter_map(|(range, role)| match role {
                Role::Token(token) => Some((&line[range], token)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn keyword_lists_are_sorted() {
        let langs = [
            Lang::Rust,
            Lang::Python,
            Lang::Script,
            Lang::Shell,
            Lang::Go,
            Lang::CLike,
            Lang::Json,
            Lang::Sql,
            Lang::Lua,
        ];
        for lang in langs {
            assert!(lang.keywords().is_sorted(), "{lang:?}");
        }
    }

    #[test]
    fn rust_line() {
        assert_eq!(
            tokens(r#"let x = "a\"b" + 42; // fin"#, Lang::Rust),
            [
                ("let", Token::Keyword),
                (r#""a\"b""#, Token::String),
                ("42", Token::Number),
                ("// fin", Token::Comment),
            ]
        );
    }

    #[test]
    fn a_hash_in_a_shell_word_is_not_a_comment() {
        assert_eq!(
            tokens("echo a#b # c", Lang::Shell),
            [("# c", Token::Comment)]
        );
    }

    #[test]
    fn multibyte_text_is_skipped_whole() {
        assert_eq!(tokens("é // ñ", Lang::Rust), [("// ñ", Token::Comment)]);
    }

    #[test]
    fn unclosed_quotes_and_names_with_digits() {
        assert_eq!(tokens("it's x1", Lang::Python), []);
        assert_eq!(tokens("SELECT a", Lang::Sql), [("SELECT", Token::Keyword)]);
    }

    #[test]
    fn languages_by_info_string() {
        assert_eq!(Lang::of("Rust ignore"), Some(Lang::Rust));
        assert_eq!(Lang::of("ts"), Some(Lang::Script));
        assert_eq!(Lang::of("mermaid"), None);
        assert_eq!(Lang::of(""), None);
    }
}
