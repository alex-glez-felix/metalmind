use std::fmt;
use std::ops::Add;

/// Posición en el texto medida en bytes UTF-8 desde el inicio del buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteOffset(usize);

impl ByteOffset {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(bytes: usize) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }

    #[must_use]
    pub const fn saturating_sub(self, bytes: usize) -> Self {
        Self(self.0.saturating_sub(bytes))
    }
}

impl Add<usize> for ByteOffset {
    type Output = Self;

    fn add(self, bytes: usize) -> Self {
        Self(self.0 + bytes)
    }
}

impl fmt::Display for ByteOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}b", self.0)
    }
}
