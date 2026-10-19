//! El texto de un buffer en un momento, para llevarlo a otro hilo.

use std::fmt;

use ropey::Rope;

/// Copiarlo no copia el texto: comparte los nodos del rope con el buffer, y
/// las ediciones que vengan después no lo cambian.
#[derive(Clone, Debug)]
pub struct Snapshot(Rope);

impl Snapshot {
    pub(crate) fn new(rope: Rope) -> Self {
        Self(rope)
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.0.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}
