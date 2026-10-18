use crate::Operator;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    OperatorPending(Operator),
    /// Escribiendo tras `:`.
    CommandLine,
}

impl Mode {
    /// Las teclas de texto se escriben en vez de ser comandos.
    #[must_use]
    pub fn takes_text(self) -> bool {
        matches!(self, Self::Insert | Self::CommandLine)
    }
}
