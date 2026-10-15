use nota_buffer::BufferError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VimError {
    #[error(transparent)]
    Buffer(#[from] BufferError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeymapError {
    #[error("tecla desconocida: {0:?}")]
    UnknownKey(String),
    #[error("acción desconocida: {0:?}")]
    UnknownAction(String),
    #[error("secuencia de teclas vacía")]
    EmptySequence,
}
