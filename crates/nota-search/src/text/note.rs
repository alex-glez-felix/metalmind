//! Buscar en una nota: sus líneas que coinciden, con cada coincidencia.

use std::fs::File;
use std::io::{self, Read as _};
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use grep_matcher::{LineTerminator, Matcher as _};
use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};

use super::SearchError;

/// Una línea de una nota con al menos una coincidencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMatch {
    /// Desde 0.
    pub line: usize,
    /// Offset en bytes del inicio de la línea en la nota.
    pub offset: usize,
    /// La línea, sin el salto (`\n` o `\r\n`): el contexto que muestra la UI.
    pub text: String,
    /// Cada coincidencia, en bytes dentro de `text`, creciente y sin solaparse.
    /// Su `start` es la columna; `offset + start`, el offset en la nota.
    pub matches: Vec<Range<usize>>,
}

/// El patrón de una [`TextQuery`](super::TextQuery) ya compilado.
#[derive(Debug, Clone)]
pub struct TextMatcher {
    regex: RegexMatcher,
}

impl TextMatcher {
    pub(super) fn new(regex: RegexMatcher) -> Self {
        Self { regex }
    }

    /// Las líneas de `text` que coinciden: la búsqueda en una nota que ya está
    /// en memoria, como la de un buffer con cambios sin guardar.
    ///
    /// # Errors
    /// Si falla el motor de búsqueda.
    pub fn find(&self, text: &str) -> Result<Vec<LineMatch>, SearchError> {
        let mut sink = Collect::new(&self.regex, None);
        searcher(BinaryDetection::none())
            .search_slice(&self.regex, text.as_bytes(), &mut sink)
            .map_err(SearchError::Engine)?;
        Ok(sink.lines)
    }
}

/// Lo que cada hilo reusa de una nota a la siguiente.
pub(super) struct NoteSearcher {
    searcher: Searcher,
    matcher: TextMatcher,
    bytes: Vec<u8>,
}

impl NoteSearcher {
    pub(super) fn new(matcher: TextMatcher) -> Self {
        Self {
            searcher: searcher(BinaryDetection::quit(b'\0')),
            matcher,
            bytes: Vec::new(),
        }
    }

    /// Las líneas de `path` que coinciden. Nada si no es texto que nota pueda
    /// abrir (tiene un NUL o no es UTF-8) o si `cancel` se encendió a medias.
    pub(super) fn search(
        &mut self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> io::Result<Vec<LineMatch>> {
        self.bytes.clear();
        File::open(path)?.read_to_end(&mut self.bytes)?;
        let regex = &self.matcher.regex;
        let mut sink = Collect::new(regex, Some(cancel));
        self.searcher.search_slice(regex, &self.bytes, &mut sink)?;
        let complete = !sink.not_text && !cancel.load(Ordering::Relaxed);
        if sink.lines.is_empty() || !complete || !can_open(&self.bytes) {
            return Ok(Vec::new());
        }
        Ok(sink.lines)
    }
}

/// Lo que nota abre: UTF-8 sin NUL. `search_slice` solo busca el NUL en los
/// primeros 64 KiB, así que se mira todo, y solo en notas que coinciden.
fn can_open(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && str::from_utf8(bytes).is_ok()
}

fn searcher(binary: BinaryDetection) -> Searcher {
    SearcherBuilder::new()
        .line_terminator(LineTerminator::crlf())
        .binary_detection(binary)
        .line_number(true)
        .build()
}

struct Collect<'a> {
    regex: &'a RegexMatcher,
    cancel: Option<&'a AtomicBool>,
    lines: Vec<LineMatch>,
    /// Se encontró un NUL o una línea que no es UTF-8.
    not_text: bool,
}

impl<'a> Collect<'a> {
    fn new(regex: &'a RegexMatcher, cancel: Option<&'a AtomicBool>) -> Self {
        Self {
            regex,
            cancel,
            lines: Vec::new(),
            not_text: false,
        }
    }

    /// Las coincidencias no vacías de `text`: un `^` o un `x*` no marcan nada.
    fn matches(&self, text: &str) -> io::Result<Vec<Range<usize>>> {
        let mut matches = Vec::new();
        self.regex
            .find_iter(text.as_bytes(), |found| {
                if !found.is_empty() {
                    matches.push(found.start()..found.end());
                }
                true
            })
            .map_err(io::Error::other)?;
        Ok(matches)
    }
}

impl Sink for Collect<'_> {
    type Error = io::Error;

    fn matched(&mut self, _: &Searcher, found: &SinkMatch<'_>) -> io::Result<bool> {
        if self
            .cancel
            .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
        {
            return Ok(false);
        }
        let first_line = found
            .line_number()
            .ok_or_else(|| io::Error::other("el searcher no numera las líneas"))?;
        let mut offset = to_usize(found.absolute_byte_offset())?;
        let first_line = to_usize(first_line)? - 1;
        for (line, raw) in (first_line..).zip(found.lines()) {
            let Ok(text) = str::from_utf8(without_terminator(raw)) else {
                self.not_text = true;
                return Ok(false);
            };
            let matches = self.matches(text)?;
            if !matches.is_empty() {
                self.lines.push(LineMatch {
                    line,
                    offset,
                    text: text.to_owned(),
                    matches,
                });
            }
            offset += raw.len();
        }
        Ok(true)
    }

    fn binary_data(&mut self, _: &Searcher, _: u64) -> io::Result<bool> {
        self.not_text = true;
        Ok(false)
    }
}

/// Sin el `\n` o el `\r\n` final; un `\r` suelto es parte del texto.
fn without_terminator(line: &[u8]) -> &[u8] {
    match line.strip_suffix(b"\n") {
        Some(line) => line.strip_suffix(b"\r").unwrap_or(line),
        None => line,
    }
}

fn to_usize(value: u64) -> io::Result<usize> {
    usize::try_from(value).map_err(io::Error::other)
}
