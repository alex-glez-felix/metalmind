//! Marca con `cfg(nvim_available)` y `cfg(valgrind_available)` si hay `nvim`
//! y `valgrind` en el PATH (ver `build/in_path.rs`).

#[path = "../../build/in_path.rs"]
mod in_path;

fn main() {
    in_path::mark_nvim();
    in_path::mark_valgrind();
}
