//! La búsqueda difusa del finder y de la paleta: una función pura de query y
//! candidatos a resultados ordenados.

use std::cmp::Reverse;

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use unicode_segmentation::UnicodeSegmentation as _;

/// Lo escrito en el finder, ya interpretado. Sigue la sintaxis de `nucleo`:
/// palabras separadas por espacio que tienen que coincidir todas.
/// Mayúsculas y acentos cuentan solo si el query los lleva: `nino`
/// encuentra `niño`, pero `ñandu` no a `ñandú`.
#[derive(Clone, Debug)]
pub struct Query(Pattern);

impl Query {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(Pattern::parse(
            text,
            CaseMatching::Smart,
            Normalization::Smart,
        ))
    }

    /// Lo que empieza con `text`, con las mismas reglas de mayúsculas y
    /// acentos que [`Query::new`]: `cancion` encuentra `canción`.
    #[must_use]
    pub fn prefix(text: &str) -> Self {
        Self(Pattern::new(
            text,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Prefix,
        ))
    }

    /// Un query vacío (o solo espacios) deja pasar todo.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.atoms.is_empty()
    }
}

/// Un candidato que coincide con el query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ranked {
    /// Su posición en los candidatos de entrada.
    pub index: usize,
    pub score: u32,
}

/// Busca como `fzf`: los caracteres del query en orden, con bonus por
/// empezar palabra. Guarda los buffers de `nucleo` entre búsquedas.
pub struct FuzzyMatcher {
    matcher: Matcher,
    chars: Vec<char>,
    indices: Vec<u32>,
}

impl FuzzyMatcher {
    /// Para rutas: bonus extra al empezar cada componente.
    #[must_use]
    pub fn paths() -> Self {
        Self::with_config(Config::DEFAULT.match_paths())
    }

    /// Para nombres sueltos, como los de las acciones de la paleta.
    #[must_use]
    pub fn text() -> Self {
        Self::with_config(Config::DEFAULT)
    }

    fn with_config(config: Config) -> Self {
        Self {
            matcher: Matcher::new(config),
            chars: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Los candidatos que coinciden con `query`, del mejor al peor; a igual
    /// puntaje, el más corto y luego el orden de entrada. Con el query vacío
    /// coinciden todos, en el orden de entrada. No calcula posiciones: la
    /// UI las pide con [`FuzzyMatcher::positions`] solo para lo que pinta.
    pub fn rank(&mut self, query: &Query, candidates: &[impl AsRef<str>]) -> Vec<Ranked> {
        if query.is_empty() {
            return (0..candidates.len())
                .map(|index| Ranked { index, score: 0 })
                .collect();
        }
        let mut ranked: Vec<Ranked> = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                let haystack = Utf32Str::new(candidate.as_ref(), &mut self.chars);
                let score = query.0.score(haystack, &mut self.matcher)?;
                Some(Ranked { index, score })
            })
            .collect();
        ranked.sort_unstable_by_key(|ranked| {
            let length = candidates[ranked.index].as_ref().len();
            (Reverse(ranked.score), length, ranked.index)
        });
        ranked
    }

    /// Offsets en bytes, crecientes, del inicio de cada grafema de
    /// `candidate` que coincide con `query`; vacío si no coincide.
    pub fn positions(&mut self, query: &Query, candidate: &str) -> Vec<usize> {
        self.indices.clear();
        let haystack = Utf32Str::new(candidate, &mut self.chars);
        let in_bytes = matches!(haystack, Utf32Str::Ascii(_));
        if query
            .0
            .indices(haystack, &mut self.matcher, &mut self.indices)
            .is_none()
        {
            return Vec::new();
        }
        // Cada palabra del query agrega sus índices por separado.
        self.indices.sort_unstable();
        self.indices.dedup();
        if in_bytes {
            self.indices.iter().map(|&offset| offset as usize).collect()
        } else {
            grapheme_offsets(candidate, &self.indices)
        }
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
