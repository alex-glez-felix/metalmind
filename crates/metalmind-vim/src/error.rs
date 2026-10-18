use metalmind_buffer::BufferError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VimError {
    #[error(transparent)]
    Buffer(#[from] BufferError),
}
