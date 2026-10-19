//! Marca con `cfg(nvim_available)` si hay `nvim` en el PATH, para que los
//! tests del oráculo se ignoren (y no pasen en falso) cuando falta, y con
//! `cfg(valgrind_available)` si hay `valgrind`, para las pruebas de
//! complejidad, que cuentan instrucciones con callgrind.

fn main() {
    println!("cargo::rerun-if-env-changed=PATH");
    mark_if_in_path("nvim", "nvim_available");
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
