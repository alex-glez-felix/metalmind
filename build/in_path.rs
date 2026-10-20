//! Lo comparten los `build.rs` de los crates (`#[path]`): marcar con un `cfg`
//! si una herramienta de los tests está en el PATH, para que los tests que la
//! necesitan se ignoren (y no pasen en falso) cuando falta.

/// Con `cfg(<cfg>)` si `program` está en algún directorio del PATH.
pub fn mark_if_in_path(program: &str, cfg: &str) {
    println!("cargo::rustc-check-cfg=cfg({cfg})");
    println!("cargo::rerun-if-env-changed=PATH");
    let found = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()));
    if found {
        println!("cargo::rustc-cfg={cfg}");
    }
}

/// `valgrind`, para las pruebas de complejidad, que cuentan instrucciones
/// con callgrind.
pub fn mark_valgrind() {
    mark_if_in_path("valgrind", "valgrind_available");
}

/// `nvim`, el oráculo de vim.
#[allow(dead_code, reason = "solo lo usan los crates con casos contra nvim")]
pub fn mark_nvim() {
    mark_if_in_path("nvim", "nvim_available");
}
