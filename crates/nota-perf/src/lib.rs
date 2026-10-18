//! Soporte de rendimiento de nota, solo para benchmarks y tests: notas
//! sintéticas deterministas (nada de datos reales), una nota abierta en el
//! motor vim y la prueba de que una operación crece linealmente con el tamaño
//! de su entrada.
//!
//! Los números objetivo y cómo se miden están en `docs/performance.md`.

mod growth;
mod session;
mod text;

pub use growth::{FACTOR, MAX_FLAT_GROWTH, MAX_LINEAR_GROWTH, assert_flat, assert_linear, growth};
pub use session::Session;
pub use text::{
    MISSING_WORD, WORD, backtick_runs, hash_runs, huge_line, note, unclosed_link_titles,
    unclosed_wikilinks, vault_paths,
};
