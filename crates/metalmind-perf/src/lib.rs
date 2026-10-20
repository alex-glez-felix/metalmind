//! Soporte de rendimiento de Metalmind, solo para benchmarks y tests: notas y
//! vaults sintéticos deterministas (nada de datos reales), una nota abierta en
//! el motor vim y la prueba de que una operación crece linealmente con el
//! tamaño de su entrada.
//!
//! Los números objetivo y cómo se miden están en `docs/performance.md`.

mod growth;
mod limits;
mod links;
mod math;
mod session;
mod tags;
mod text;
mod vault;

pub use growth::{FACTOR, MAX_FLAT_GROWTH, MAX_LINEAR_GROWTH, assert_flat, assert_linear, growth};
pub use limits::{MAX_INSTRUCTION_REGRESSION, MAX_NOISY_INSTRUCTION_REGRESSION};
pub use links::{NoteNames, linked_vault};
pub use math::{FORMULAS, Formula, dollar_runs, formulas_note, math_note};
pub use session::Session;
pub use tags::{tagged_vault, write_files};
pub use text::{
    MISSING_WORD, WORD, backtick_runs, hash_runs, huge_line, note, unclosed_link_titles,
    unclosed_wikilinks, vault_paths,
};
pub use vault::{RARE_EVERY, RARE_WORD, vault_note, write_vault};
