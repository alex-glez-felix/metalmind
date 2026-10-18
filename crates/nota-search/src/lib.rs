//! Búsqueda del vault. Por ahora, la difusa de rutas del finder: una función
//! pura de query y candidatos a resultados ordenados.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use unicode_segmentation::UnicodeSegmentation as _;

/// Un candidato que coincide con el query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMatch {
    /// Su posición en los candidatos de entrada.
    pub index: usize,
    pub score: u32,
    /// Offsets en bytes, crecientes, del inicio de cada grafema que coincide.
    pub positions: Vec<usize>,
}

/// Busca rutas como `fzf`: los caracteres del query en orden, con bonus por
/// empezar palabra o componente. Guarda los buffers de `nucleo` entre búsquedas.
pub struct PathMatcher {
    matcher: Matcher,
    chars: Vec<char>,
    indices: Vec<u32>,
}

impl Default for PathMatcher {
    fn default() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            chars: Vec::new(),
            indices: Vec::new(),
        }
    }
}

impl PathMatcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Los candidatos que coinciden con `query`, del mejor al peor; a igual
    /// puntaje, la ruta más corta y luego el orden de entrada. Con el query
    /// vacío coinciden todos, en el orden de entrada.
    ///
    /// El query sigue la sintaxis de `nucleo`: palabras separadas por espacio
    /// que tienen que coincidir todas. Mayúsculas y acentos cuentan solo si el
    /// query los lleva: `nino` encuentra `niño`, pero `ñandu` no a `ñandú`.
    pub fn search<'a>(
        &mut self,
        query: &str,
        candidates: impl IntoIterator<Item = &'a str>,
    ) -> Vec<PathMatch> {
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        if pattern.atoms.is_empty() {
            return candidates
                .into_iter()
                .enumerate()
                .map(|(index, _)| PathMatch {
                    index,
                    score: 0,
                    positions: Vec::new(),
                })
                .collect();
        }
        let mut found: Vec<(PathMatch, usize)> = candidates
            .into_iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                let found = self.match_one(&pattern, index, candidate)?;
                Some((found, candidate.len()))
            })
            .collect();
        found.sort_by(|(a, a_len), (b, b_len)| {
            b.score
                .cmp(&a.score)
                .then(a_len.cmp(b_len))
                .then(a.index.cmp(&b.index))
        });
        found.into_iter().map(|(found, _)| found).collect()
    }

    fn match_one(&mut self, pattern: &Pattern, index: usize, candidate: &str) -> Option<PathMatch> {
        self.indices.clear();
        let haystack = Utf32Str::new(candidate, &mut self.chars);
        let in_bytes = matches!(haystack, Utf32Str::Ascii(_));
        let score = pattern.indices(haystack, &mut self.matcher, &mut self.indices)?;
        // Cada palabra del query agrega sus índices por separado.
        self.indices.sort_unstable();
        self.indices.dedup();
        let positions = if in_bytes {
            self.indices.iter().map(|&offset| offset as usize).collect()
        } else {
            grapheme_offsets(candidate, &self.indices)
        };
        Some(PathMatch {
            index,
            score,
            positions,
        })
    }
}

/// `nucleo` cuenta en bytes si cada grafema empieza con ASCII y si no, en
/// grafemas; la UI siempre en bytes.
fn grapheme_offsets(candidate: &str, graphemes: &[u32]) -> Vec<usize> {
    let mut wanted = graphemes.iter().map(|&index| index as usize).peekable();
    let mut offsets = Vec::with_capacity(graphemes.len());
    for (position, (offset, _)) in candidate.grapheme_indices(true).enumerate() {
        if wanted.next_if_eq(&position).is_some() {
            offsets.push(offset);
        }
        if wanted.peek().is_none() {
            break;
        }
    }
    offsets
}
