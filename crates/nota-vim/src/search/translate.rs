//! El dialecto de vim (`magic`) en la sintaxis de `regex`. Lo que no tiene
//! equivalente directo se rechaza en vez de buscar otra cosa.

use super::SearchError;

/// El patrón ya traducido y lo que pidieron `\c` o `\C`.
pub(super) struct Translation {
    pub regex: String,
    /// `Some(true)` con `\c`, `Some(false)` con `\C`.
    pub ignore_case: Option<bool>,
}

pub(super) fn translate(source: &str) -> Result<Translation, SearchError> {
    let mut translator = Translator {
        chars: source.chars().collect(),
        next: 0,
        regex: String::new(),
        atom: false,
        branch_start: true,
        ignore_case: None,
    };
    while let Some(c) = translator.bump() {
        translator.token(c)?;
    }
    Ok(Translation {
        regex: translator.regex,
        ignore_case: translator.ignore_case,
    })
}

/// Como `pat_has_uppercase()` de vim: lo escapado no cuenta (`\S` no es una
/// mayúscula).
pub(super) fn has_uppercase(source: &str) -> bool {
    let mut chars = source.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some('_' | '%') = chars.next() {
                chars.next();
            }
        } else if c.is_uppercase() {
            return true;
        }
    }
    false
}

struct Translator {
    chars: Vec<char>,
    next: usize,
    regex: String,
    /// Lo anterior se puede repetir con `*`, `\+`, `\{n,m}`…
    atom: bool,
    /// Inicio del patrón o de una rama, donde `^` es ancla.
    branch_start: bool,
    ignore_case: Option<bool>,
}

impl Translator {
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.next).copied()?;
        self.next += 1;
        Some(c)
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.next + ahead).copied()
    }

    fn token(&mut self, c: char) -> Result<(), SearchError> {
        match c {
            '\\' => self.escaped()?,
            '^' if self.branch_start => self.assertion("^"),
            '$' if self.at_branch_end() => self.assertion("$"),
            '.' => self.atom("."),
            '*' => self.quantifier("*", '*'),
            '[' => self.collection()?,
            c => self.literal(c),
        }
        Ok(())
    }

    fn escaped(&mut self) -> Result<(), SearchError> {
        let Some(c) = self.bump() else {
            self.literal('\\');
            return Ok(());
        };
        match c {
            '<' => self.assertion(r"\b{start}"),
            '>' => self.assertion(r"\b{end}"),
            '(' => self.open_group("("),
            '%' if self.peek(0) == Some('(') => {
                self.next += 1;
                self.open_group("(?:");
            }
            '%' => {
                let next = self.peek(0).map(String::from).unwrap_or_default();
                return Err(unsupported(&format!("\\%{next}")));
            }
            ')' => self.atom(")"),
            '|' => {
                self.regex.push('|');
                self.atom = false;
                self.branch_start = true;
            }
            '+' => self.quantifier("+", '+'),
            '=' | '?' => self.quantifier("?", c),
            '{' => self.braces()?,
            'c' => self.ignore_case = Some(true),
            'C' => {
                self.ignore_case.get_or_insert(false);
            }
            's' => self.atom("[ \\t]"),
            'S' => self.atom("[^ \\t\\n]"),
            'd' => self.atom("[0-9]"),
            'D' => self.atom("[^0-9\\n]"),
            'w' => self.atom("[0-9A-Za-z_]"),
            'W' => self.atom("[^0-9A-Za-z_\\n]"),
            't' => self.atom("\\t"),
            'e' => self.atom("\\x1b"),
            'r' => self.atom("\\r"),
            c if c.is_ascii_punctuation() => self.literal(c),
            c => return Err(unsupported(&format!("\\{c}"))),
        }
        Ok(())
    }

    /// `$` es ancla solo al final del patrón o de una rama.
    fn at_branch_end(&self) -> bool {
        match self.peek(0) {
            None => true,
            Some('\\') => matches!(self.peek(1), Some('|' | ')')),
            Some(_) => false,
        }
    }

    fn open_group(&mut self, group: &str) {
        self.regex.push_str(group);
        self.atom = false;
        self.branch_start = true;
    }

    fn atom(&mut self, regex: &str) {
        self.regex.push_str(regex);
        self.atom = true;
        self.branch_start = false;
    }

    fn assertion(&mut self, regex: &str) {
        self.regex.push_str(regex);
        self.atom = false;
        self.branch_start = false;
    }

    fn literal(&mut self, c: char) {
        let mut buffer = [0; 4];
        self.atom(&regex::escape(c.encode_utf8(&mut buffer)));
    }

    /// Sin nada que repetir, vim toma el `*` (o el `\+`) como texto.
    fn quantifier(&mut self, regex: &str, otherwise: char) {
        if self.atom {
            self.assertion(regex);
        } else {
            self.literal(otherwise);
        }
    }

    /// `\{n,m}`, `\{n}`, `\{n,}`, `\{,m}`, `\{}` y sus versiones perezosas
    /// con `-`; vim acepta el cierre como `}` o `\}`.
    fn braces(&mut self) -> Result<(), SearchError> {
        let mut body = String::new();
        loop {
            match self.bump() {
                Some('}') => break,
                Some('\\') if self.peek(0) == Some('}') => {
                    self.next += 1;
                    break;
                }
                Some(c) if c.is_ascii_digit() || c == ',' || c == '-' => body.push(c),
                _ => return Err(invalid(&self.chars)),
            }
        }
        let (lazy, bounds) = body
            .strip_prefix('-')
            .map_or((false, body.as_str()), |rest| (true, rest));
        let repetition = match bounds.split_once(',') {
            None if bounds.is_empty() => "*".to_owned(),
            None => format!("{{{bounds}}}"),
            Some(("", max)) => format!("{{0,{max}}}"),
            Some((min, max)) => format!("{{{min},{max}}}"),
        };
        if !self.atom || bounds.contains('-') {
            return Err(invalid(&self.chars));
        }
        self.assertion(&repetition);
        if lazy {
            self.regex.push('?');
        }
        Ok(())
    }

    /// `[…]`; sin `]` que la cierre, vim busca el `[` como texto. Como en
    /// vim sin `\_`, nunca incluye el `\n`, ni negada ni con `[:space:]`.
    fn collection(&mut self) -> Result<(), SearchError> {
        let Some(end) = collection_end(&self.chars, self.next) else {
            self.literal('[');
            return Ok(());
        };
        let mut regex = String::from("[");
        let mut at = self.next;
        if self.chars[at] == '^' {
            regex.push('^');
            at += 1;
        }
        if self.chars[at] == ']' {
            regex.push_str("\\]");
            at += 1;
        }
        while at < end {
            at = self.collection_item(at, &mut regex)?;
        }
        regex.push(']');
        self.next = end + 1;
        self.atom(&format!("[{regex}&&[^\\n]]"));
        Ok(())
    }

    /// Traduce lo que empieza en `at` dentro de `[…]` y dice dónde sigue.
    fn collection_item(&self, at: usize, regex: &mut String) -> Result<usize, SearchError> {
        let chars = &self.chars;
        match (chars[at], chars.get(at + 1).copied()) {
            ('[', Some(':')) => {
                if let Some(close) = class_name_end(chars, at + 2) {
                    regex.extend(&chars[at..close]);
                    return Ok(close);
                }
                regex.push_str("\\[");
            }
            ('\\', Some(c @ (']' | '^' | '-' | '\\'))) => {
                regex.push('\\');
                regex.push(c);
                return Ok(at + 2);
            }
            ('\\', Some('t')) => {
                regex.push_str("\\t");
                return Ok(at + 2);
            }
            ('\\', Some('e')) => {
                regex.push_str("\\x1b");
                return Ok(at + 2);
            }
            ('\\', Some(c @ ('n' | 'r' | 'b' | 'd' | 'o' | 'x' | 'u' | 'U'))) => {
                return Err(unsupported(&format!("[\\{c}]")));
            }
            // Otra barra es texto, como en vim: `[\xyz]` incluye la `\`.
            ('\\', _) => regex.push_str("\\\\"),
            // `regex` usa `[`, `&&` y `~~` para operar conjuntos.
            (c @ ('[' | '&' | '~'), _) => {
                regex.push('\\');
                regex.push(c);
            }
            (c, _) => regex.push(c),
        }
        Ok(at + 1)
    }
}

/// Índice del `]` que cierra una colección que empieza en `start`.
fn collection_end(chars: &[char], start: usize) -> Option<usize> {
    let mut at = start;
    if chars.get(at) == Some(&'^') {
        at += 1;
    }
    if chars.get(at) == Some(&']') {
        at += 1;
    }
    while let Some(&c) = chars.get(at) {
        match c {
            ']' => return Some(at),
            '\\' => at += 2,
            '[' if chars.get(at + 1) == Some(&':') => {
                at = class_name_end(chars, at + 2).unwrap_or(at + 1);
            }
            _ => at += 1,
        }
    }
    None
}

/// Tras `[:`, el índice que sigue a `:]` si el nombre es de letras.
fn class_name_end(chars: &[char], start: usize) -> Option<usize> {
    let mut at = start;
    while chars.get(at).is_some_and(char::is_ascii_alphabetic) {
        at += 1;
    }
    (chars.get(at) == Some(&':') && chars.get(at + 1) == Some(&']')).then_some(at + 2)
}

fn unsupported(construct: &str) -> SearchError {
    SearchError::Unsupported(construct.to_owned())
}

fn invalid(chars: &[char]) -> SearchError {
    SearchError::Invalid(chars.iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regex(source: &str) -> String {
        translate(source).unwrap().regex
    }

    #[test]
    fn translates_magic_syntax() {
        let cases = [
            ("foo", "foo"),
            ("a.c", "a.c"),
            ("ab*", "ab*"),
            ("*a", "\\*a"),
            ("^a*", "^a*"),
            ("^*", "^\\*"),
            ("a^b", "a\\^b"),
            ("a$", "a$"),
            ("a$b", "a\\$b"),
            ("a\\|^b$\\|c", "a|^b$|c"),
            ("\\(a\\)\\+", "(a)+"),
            ("\\%(a\\)\\=", "(?:a)?"),
            ("a\\?", "a?"),
            ("\\<foo\\>", "\\b{start}foo\\b{end}"),
            ("a\\{2,3}", "a{2,3}"),
            ("a\\{,3\\}", "a{0,3}"),
            ("a\\{-}", "a*?"),
            ("a\\{-1,}", "a{1,}?"),
            ("a+b?(c){d}|e", "a\\+b\\?\\(c\\)\\{d\\}\\|e"),
            ("\\/\\.\\*\\~", "/\\.\\*\\~"),
            ("~", "\\~"),
            ("\\s\\d\\w", "[ \\t][0-9][0-9A-Za-z_]"),
            ("\\S\\D\\W", "[^ \\t\\n][^0-9\\n][^0-9A-Za-z_\\n]"),
            ("[abc]", "[[abc]&&[^\\n]]"),
            ("[^a-z]", "[[^a-z]&&[^\\n]]"),
            ("[]a]", "[[\\]a]&&[^\\n]]"),
            ("[^]a]", "[[^\\]a]&&[^\\n]]"),
            ("[[:alpha:]_]", "[[[:alpha:]_]&&[^\\n]]"),
            ("[a\\]b]", "[[a\\]b]&&[^\\n]]"),
            ("[\\zy]", "[[\\\\zy]&&[^\\n]]"),
            ("[a&&b~~c]", "[[a\\&\\&b\\~\\~c]&&[^\\n]]"),
            ("[ab", "\\[ab"),
            ("a[", "a\\["),
            ("ñ.é", "ñ.é"),
        ];
        for (source, expected) in cases {
            assert_eq!(regex(source), expected, "{source:?}");
        }
    }

    #[test]
    fn case_flags_leave_the_pattern() {
        let translation = translate("\\Cfoo\\c").unwrap();
        assert_eq!(translation.regex, "foo");
        assert_eq!(translation.ignore_case, Some(true));
        let translation = translate("f\\Coo").unwrap();
        assert_eq!(translation.ignore_case, Some(false));
    }

    #[test]
    fn rejects_what_has_no_equivalent() {
        for source in ["a\\nb", "\\zsa", "\\va", "\\1", "\\%V", "[\\n]", "\\l"] {
            assert!(
                matches!(translate(source), Err(SearchError::Unsupported(_))),
                "{source:?}"
            );
        }
        for source in ["a\\{x}", "\\{2}", "a\\{2"] {
            assert!(
                matches!(translate(source), Err(SearchError::Invalid(_))),
                "{source:?}"
            );
        }
    }

    #[test]
    fn uppercase_counts_only_unescaped() {
        assert!(has_uppercase("fOo"));
        assert!(!has_uppercase("foo\\S\\W"));
        assert!(!has_uppercase("\\%Vfoo"));
        assert!(has_uppercase("Ñ"));
    }
}
