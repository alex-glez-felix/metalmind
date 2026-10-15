//! Marca con `cfg(nvim_available)` si hay `nvim` en el PATH, para que los
//! tests del oráculo se ignoren (y no pasen en falso) cuando falta.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(nvim_available)");
    println!("cargo::rerun-if-env-changed=PATH");
    let found = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("nvim").is_file()));
    if found {
        println!("cargo::rustc-cfg=nvim_available");
    }
}
