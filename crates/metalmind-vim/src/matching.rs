//! `%` sin count: el paréntesis, corchete o llave que hace pareja con el
//! primero que haya desde el cursor en su línea. Port de `findmatchlimit`
//! de nvim 0.12.6 con `matchpairs` y `cpoptions` por default: ignora lo que
//! está entre comillas en una línea con un número par de ellas y lo escapado
//! con `\`, salvo que el de partida también lo esté. No salta entre `/*` y
//! `*/` ni entre `#if` y `#endif`, que en markdown serían texto o headings.

use std::borrow::Cow;

use metalmind_buffer::{Buffer, ByteOffset};

/// La pareja de `c` y si se busca hacia atrás.
fn pair(c: u8) -> Option<(u8, bool)> {
    match c {
        b'(' => Some((b')', false)),
        b'[' => Some((b']', false)),
        b'{' => Some((b'}', false)),
        b')' => Some((b'(', true)),
        b']' => Some((b'[', true)),
        b'}' => Some((b'{', true)),
        _ => None,
    }
}

/// `None` si no hay nada que emparejar en la línea o no tiene pareja.
pub(crate) fn find(buffer: &Buffer, head: ByteOffset) -> Option<ByteOffset> {
    let line = buffer.line_of(head);
    let text = buffer.line_text(line);
    let mut col = head.get() - buffer.line_start(line).get();
    if col == text.len() && col > 0 {
        col = prev_char(&text, col);
    }
    let start = text.as_bytes()[col..]
        .iter()
        .position(|&c| pair(c).is_some())
        .map(|found| col + found)?;
    let initc = text.as_bytes()[start];
    let (findc, backwards) = pair(initc)?;
    let mut search = Search {
        buffer,
        initc,
        findc,
        backwards,
        match_escaped: !backslashes_before(text.as_bytes(), start).is_multiple_of(2),
        line,
        col: start,
        text,
        quotes: None,
        start_in_quotes: None,
        inquote: false,
        count: 0,
    };
    let (line, col) = search.run()?;
    Some(buffer.line_start(line) + col)
}

struct Search<'a> {
    buffer: &'a Buffer,
    initc: u8,
    findc: u8,
    backwards: bool,
    match_escaped: bool,
    line: usize,
    col: usize,
    text: Cow<'a, str>,
    /// Si la línea tiene un número par de comillas; `None` hasta contarlas.
    quotes: Option<bool>,
    start_in_quotes: Option<bool>,
    inquote: bool,
    count: usize,
}

impl Search<'_> {
    fn run(&mut self) -> Option<(usize, usize)> {
        loop {
            self.advance()?;
            if self.quotes.is_none() {
                self.count_quotes();
            }
            self.start_in_quotes.get_or_insert(false);
            if let Some(found) = self.check() {
                return Some(found);
            }
        }
    }

    fn advance(&mut self) -> Option<()> {
        if self.backwards {
            if self.col == 0 {
                self.line = self.line.checked_sub(1)?;
                self.enter_line();
                self.col = self.text.len();
            } else {
                self.col = prev_char(&self.text, self.col);
            }
        } else if self.col >= self.text.len() {
            if self.line == self.buffer.last_line() {
                return None;
            }
            self.line += 1;
            self.enter_line();
            self.col = 0;
        } else {
            self.col += char_len(&self.text, self.col);
        }
        Some(())
    }

    fn enter_line(&mut self) {
        self.text = self.buffer.line_text(self.line);
        self.quotes = None;
    }

    /// Cuenta las comillas de la línea, sin las escapadas ni `'"'`. Con un
    /// número impar no se sabe qué está entre comillas, salvo que esta línea
    /// o la anterior terminen en `\`.
    fn count_quotes(&mut self) {
        let bytes = self.text.as_bytes();
        // Como vim, que cuenta desde -1.
        let mut quotes: isize = -1;
        let mut at_start = -1;
        let here = self.col + usize::from(self.backwards);
        let mut i = 0;
        while i < bytes.len() {
            if i == here {
                at_start = quotes & 1;
            }
            let quoted_quote = i > 0 && bytes[i - 1] == b'\'' && bytes.get(i + 1) == Some(&b'\'');
            if bytes[i] == b'"' && !quoted_quote {
                quotes += 1;
            }
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 1;
            }
            i += 1;
        }
        let even = quotes & 1 == 1;
        self.quotes = Some(even);
        if even {
            return;
        }
        self.inquote = false;
        if bytes.last() == Some(&b'\\') {
            self.quotes = Some(true);
            if self.start_in_quotes.is_none() {
                self.inquote = true;
                self.start_in_quotes = Some(true);
            } else if self.backwards {
                self.inquote = true;
            }
        }
        let continued = self.line > 0
            && self
                .buffer
                .line_text(self.line - 1)
                .as_bytes()
                .last()
                .is_some_and(|&c| c == b'\\');
        if continued {
            self.quotes = Some(true);
            if self.start_in_quotes.is_none() {
                self.inquote = at_start != 0;
                if self.inquote {
                    self.start_in_quotes = Some(true);
                }
            } else if !self.backwards {
                self.inquote = true;
            }
        }
    }

    /// Lo que hace el carácter de `col`; la posición si es la pareja.
    fn check(&mut self) -> Option<(usize, usize)> {
        let bytes = self.text.as_bytes();
        let col = self.col;
        let Some(&c) = bytes.get(col) else {
            if col == 0 || bytes[col - 1] != b'\\' {
                self.inquote = false;
                self.start_in_quotes = Some(false);
            }
            return None;
        };
        match c {
            b'"' => {
                if self.quotes == Some(true) && backslashes_before(bytes, col).is_multiple_of(2) {
                    self.inquote = !self.inquote;
                    self.start_in_quotes = Some(false);
                }
                None
            }
            b'\'' if self.skip_char_literal() => None,
            c if c == self.initc || c == self.findc => self.bracket(c),
            _ => None,
        }
    }

    /// `'x'` o `'\x'`: se saltan, sin confundir el apóstrofo de "jon's".
    fn skip_char_literal(&mut self) -> bool {
        let bytes = self.text.as_bytes();
        let col = self.col;
        let at = |i: usize| bytes.get(i).copied();
        let skip = if self.backwards {
            if col > 1 && at(col - 2) == Some(b'\'') {
                Some(col - 2)
            } else if col > 2 && at(col - 2) == Some(b'\\') && at(col - 3) == Some(b'\'') {
                Some(col - 3)
            } else {
                None
            }
        } else if at(col + 1).is_none() {
            None
        } else if at(col + 1) == Some(b'\\') && at(col + 2).is_some() && at(col + 3) == Some(b'\'')
        {
            Some(col + 3)
        } else if at(col + 2) == Some(b'\'') {
            Some(col + 2)
        } else {
            None
        };
        skip.map(|col| self.col = col).is_some()
    }

    fn bracket(&mut self, c: u8) -> Option<(usize, usize)> {
        let outside = !self.inquote || self.start_in_quotes == Some(true);
        let escaped = !backslashes_before(self.text.as_bytes(), self.col).is_multiple_of(2);
        if !outside || escaped != self.match_escaped {
            return None;
        }
        if c == self.initc {
            self.count += 1;
            return None;
        }
        if self.count == 0 {
            return Some((self.line, self.col));
        }
        self.count -= 1;
        None
    }
}

fn backslashes_before(bytes: &[u8], col: usize) -> usize {
    bytes[..col]
        .iter()
        .rev()
        .take_while(|&&c| c == b'\\')
        .count()
}

fn prev_char(text: &str, col: usize) -> usize {
    text[..col].char_indices().next_back().map_or(0, |(i, _)| i)
}

fn char_len(text: &str, col: usize) -> usize {
    text[col..].chars().next().map_or(1, char::len_utf8)
}
