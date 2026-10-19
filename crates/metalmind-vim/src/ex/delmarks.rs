//! `:delm {marks}` y `:delm!`, como `ex_delmarks` de vim.

use crate::CommandError;

/// Qué marks borra un `:delm`, un bit por letra.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DeleteMarks {
    /// `a`–`z`.
    named: u32,
    /// `A`–`Z`.
    file: u32,
    /// Como vim, lo que no es una mark corta la lista, pero las anteriores
    /// ya se borraron.
    pub error: Option<CommandError>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Named,
    File,
    /// `0`–`9`: Metalmind no las tiene, así que no hay nada que borrar.
    Numbered,
}

impl Kind {
    fn of(c: char) -> Option<Self> {
        match c {
            'a'..='z' => Some(Self::Named),
            'A'..='Z' => Some(Self::File),
            '0'..='9' => Some(Self::Numbered),
            _ => None,
        }
    }
}

impl DeleteMarks {
    pub fn parse(bang: bool, args: &str) -> Result<Self, CommandError> {
        // Como en los comandos `TRLBAR` de vim, `"` empieza un comentario.
        let args = args.split_once('"').map_or(args, |(args, _)| args);
        match (bang, args.trim_start_matches(' ').is_empty()) {
            (true, true) => Ok(Self {
                named: u32::MAX >> 6,
                ..Self::default()
            }),
            (true, false) => Err(CommandError::InvalidArgument),
            (false, true) => Err(CommandError::ArgumentRequired),
            (false, false) => Ok(Self::listed(args)),
        }
    }

    fn listed(args: &str) -> Self {
        let mut marks = Self::default();
        let mut rest = args;
        while let Some(first) = rest.chars().next() {
            let after = &rest[first.len_utf8()..];
            let Some(kind) = Kind::of(first) else {
                match first {
                    ' ' | '^' | '.' | '[' | ']' => {
                        rest = after;
                        continue;
                    }
                    '<' | '>' => marks.error = Some(CommandError::UnsupportedDeleteMark(first)),
                    _ => marks.error = Some(CommandError::InvalidArgumentAt(rest.to_owned())),
                }
                return marks;
            };
            let (last, next) = match after.strip_prefix('-') {
                Some(range) => match range.chars().next() {
                    Some(last) if Kind::of(last) == Some(kind) && last >= first => {
                        (last, &range[last.len_utf8()..])
                    }
                    _ => {
                        marks.error = Some(CommandError::InvalidArgumentAt(rest.to_owned()));
                        return marks;
                    }
                },
                None => (first, after),
            };
            marks.add(kind, first, last);
            rest = next;
        }
        marks
    }

    fn add(&mut self, kind: Kind, first: char, last: char) {
        let (bits, base) = match kind {
            Kind::Named => (&mut self.named, 'a'),
            Kind::File => (&mut self.file, 'A'),
            Kind::Numbered => return,
        };
        for c in first..=last {
            *bits |= 1 << (u32::from(c) - u32::from(base));
        }
    }

    /// Los índices de las `a`–`z` que borra.
    pub fn named(&self) -> impl Iterator<Item = u8> + use<> {
        letters(self.named)
    }

    /// Los índices de las `A`–`Z` que borra.
    pub fn file(&self) -> impl Iterator<Item = u8> + use<> {
        letters(self.file)
    }
}

fn letters(bits: u32) -> impl Iterator<Item = u8> {
    (0..26).filter(move |letter| bits & (1 << letter) != 0)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn parse(bang: bool, args: &str) -> (String, Option<CommandError>) {
        let marks = DeleteMarks::parse(bang, args).unwrap();
        let named = marks.named().map(|letter| char::from(b'a' + letter));
        let file = marks.file().map(|letter| char::from(b'A' + letter));
        (named.chain(file).collect(), marks.error)
    }

    #[test]
    fn borra_letras_y_rangos_de_la_misma_clase() {
        assert_eq!(parse(false, "a"), ("a".to_owned(), None));
        assert_eq!(parse(false, "a-d"), ("abcd".to_owned(), None));
        assert_eq!(parse(false, "x b A-C"), ("bxABC".to_owned(), None));
        assert_eq!(parse(false, "aa"), ("a".to_owned(), None));
        assert_eq!(parse(false, "0-9 ^.[]"), (String::new(), None));
        assert_eq!(parse(false, "a \"b"), ("a".to_owned(), None));
    }

    #[test]
    fn con_bang_borra_todas_las_de_buffer() {
        let (marks, error) = parse(true, "");
        assert_eq!(marks, ('a'..='z').collect::<String>());
        assert_eq!(error, None);
    }

    /// Como vim, lo de antes del error queda borrado.
    #[test]
    fn lo_que_no_es_una_mark_corta_la_lista() {
        let at = |rest: &str| Some(CommandError::InvalidArgumentAt(rest.to_owned()));
        assert_eq!(parse(false, "ab?c"), ("ab".to_owned(), at("?c")));
        assert_eq!(parse(false, "b-a"), (String::new(), at("b-a")));
        assert_eq!(parse(false, "a-Z"), (String::new(), at("a-Z")));
        assert_eq!(parse(false, "a-"), (String::new(), at("a-")));
        assert_eq!(parse(false, "'"), (String::new(), at("'")));
        assert_eq!(
            parse(false, "a<"),
            (
                "a".to_owned(),
                Some(CommandError::UnsupportedDeleteMark('<'))
            )
        );
    }

    #[test]
    fn sin_marks_o_con_bang_y_marks_no_vale() {
        assert_eq!(
            DeleteMarks::parse(false, " "),
            Err(CommandError::ArgumentRequired)
        );
        assert_eq!(
            DeleteMarks::parse(false, "\"a"),
            Err(CommandError::ArgumentRequired)
        );
        assert_eq!(
            DeleteMarks::parse(true, "a"),
            Err(CommandError::InvalidArgument)
        );
    }
}
