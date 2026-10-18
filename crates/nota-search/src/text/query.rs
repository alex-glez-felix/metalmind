use std::collections::HashSet;
use std::sync::Arc;

use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use nota_vault::VaultPath;

/// Cómo se lee el patrón.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Syntax {
    /// El texto tal cual: `a.b` busca un punto.
    #[default]
    Literal,
    /// La sintaxis del crate `regex`, una línea a la vez: `^` y `$` son inicio
    /// y fin de línea, y nada coincide con el salto.
    Regex,
}

/// Cómo trata las mayúsculas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Case {
    /// Como `ignorecase` + `smartcase` en la búsqueda del buffer: distingue
    /// solo si el patrón tiene mayúsculas, y lo escapado (`\S`) no cuenta.
    #[default]
    Smart,
    Sensitive,
    Insensitive,
}

/// Una búsqueda de texto completo.
#[derive(Debug, Clone, Default)]
pub struct TextQuery {
    /// Vacío no busca nada.
    pub pattern: String,
    pub syntax: Syntax,
    pub case: Case,
    /// Si está, solo se busca en estas notas: el resultado de filtrar por
    /// tags o por el focus.
    pub scope: Option<Arc<HashSet<VaultPath>>>,
}

impl TextQuery {
    pub(super) fn matcher(&self) -> Result<RegexMatcher, grep_regex::Error> {
        let mut builder = RegexMatcherBuilder::new();
        builder
            .fixed_strings(self.syntax == Syntax::Literal)
            .line_terminator(Some(b'\n'))
            .crlf(true)
            .case_smart(self.case == Case::Smart)
            .case_insensitive(self.case == Case::Insensitive);
        builder.build(&self.pattern)
    }
}
