use crate::Operator;

/// Cómo abarca una selección Visual: por caracteres (`v`) o por líneas (`V`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualKind {
    Char,
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    OperatorPending(Operator),
    /// Cada selección va de `anchor` a `head`, los dos incluidos (ver el
    /// README del crate).
    Visual(VisualKind),
    /// Escribiendo tras `:`, `/` o `?` (ver [`Prompt`]).
    CommandLine,
}

impl Mode {
    /// Las teclas de texto se escriben en vez de ser comandos.
    #[must_use]
    pub fn takes_text(self) -> bool {
        matches!(self, Self::Insert | Self::CommandLine)
    }
}

/// Qué se escribe en la línea de comandos, para mostrar su símbolo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    /// Tras `:`.
    Command,
    /// Tras `/`, o `?` si `backward`.
    Search { backward: bool },
}

impl Prompt {
    #[must_use]
    pub fn symbol(self) -> char {
        match self {
            Self::Command => ':',
            Self::Search { backward: false } => '/',
            Self::Search { backward: true } => '?',
        }
    }
}
