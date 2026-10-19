//! Marca con `cfg(valgrind_available)` si hay `valgrind` en el PATH (ver
//! `build/in_path.rs`).

#[path = "../../build/in_path.rs"]
mod in_path;

fn main() {
    in_path::mark_valgrind();
}
