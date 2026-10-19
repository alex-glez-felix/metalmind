//! Marca con `cfg(valgrind_available)` si hay `valgrind` en el PATH, para que
//! las pruebas de complejidad, que cuentan instrucciones con callgrind, se
//! ignoren (y no pasen en falso) cuando falta.

fn main() {
    println!("cargo::rerun-if-env-changed=PATH");
    mark_if_in_path("valgrind", "valgrind_available");
}

/// Con `cfg(<cfg>)` si `program` está en algún directorio del PATH.
fn mark_if_in_path(program: &str, cfg: &str) {
    println!("cargo::rustc-check-cfg=cfg({cfg})");
    let found = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()));
    if found {
        println!("cargo::rustc-cfg={cfg}");
    }
}
