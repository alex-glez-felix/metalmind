#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Smoke de la app real: abre `nota` en un Xvfb, teclea y mira el disco.
//! Están en `ignore` porque necesitan Xvfb y Vulkan; se corren con
//! `cargo nextest run -p nota --test gui_smoke --run-ignored only`
//! (ver `docs/testing.md`).

mod support;

use rstest::rstest;
use support::Smoke;

/// Teclea `keys` sobre una nota que dice `before` y espera que salga limpio
/// con la nota diciendo `after`, sin temporales al lado.
#[rstest]
#[case::busqueda(
    "uno\ndos palabra tres\n",
    "/palabra<CR>cwPALABRA<Esc>:wq<CR>",
    "uno\ndos PALABRA tres\n"
)]
#[case::sustitucion("casa\nmala\n", ":%s/a/b/g<CR>:wq<CR>", "cbsb\nmblb\n")]
#[case::registro_con_nombre("uno\ndos\n", "\"ayyjdd\"ap:wq<CR>", "uno\nuno\n")]
#[case::macro_("a\nb\nc\n", "qaA!<Esc>jq@a:wq<CR>", "a!\nb!\nc\n")]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn edita_y_guarda(#[case] before: &str, #[case] keys: &str, #[case] after: &str) {
    let smoke = Smoke::new();
    smoke.write("a.md", before);
    let mut nota = smoke.launch(&["a.md"]);
    nota.window().type_keys(keys);
    assert!(nota.wait_exit().success());
    assert_eq!(smoke.read("a.md"), after);
    assert_eq!(smoke.files(), ["a.md"]);
}

/// Sin adaptador no hay ventana: dice qué falta y sale, no se queda corriendo (#31).
#[test]
#[ignore = "necesita Xvfb: ver docs/testing.md"]
fn sin_adaptador_termina_con_error() {
    let smoke = Smoke::new();
    let mut nota = smoke.launch_without_gpu(&["a.md"]);
    let status = nota.wait_exit();
    let log = nota.log();
    assert_eq!(status.code(), Some(1), "{log}");
    assert!(log.contains("no se pudo abrir la ventana"), "{log}");
    assert!(log.contains("vulkaninfo"), "{log}");
}

#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn wq_crea_un_archivo_nuevo() {
    let smoke = Smoke::new();
    let mut nota = smoke.launch(&["nueva.md"]);
    nota.window().type_keys("ihola<Esc>:wq<CR>");
    assert!(nota.wait_exit().success());
    assert_eq!(smoke.read("nueva.md"), "hola\n");
    assert_eq!(smoke.files(), ["nueva.md"]);
}

/// Si `:q` hubiera cerrado, el `:w` que lo sigue nunca llegaría al disco.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn q_con_cambios_se_niega() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    let mut nota = smoke.launch(&["a.md"]);
    let window = nota.window();
    window.type_keys("ix<Esc>:q<CR>:w<CR>");
    smoke.wait_contents("a.md", "xuno\n");
    assert!(nota.is_running());
    window.type_keys(":q<CR>");
    assert!(nota.wait_exit().success());
    assert_eq!(smoke.files(), ["a.md"]);
}

#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn el_cierre_del_gestor_de_ventanas_se_niega_con_cambios() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    let mut nota = smoke.launch(&["a.md"]);
    let window = nota.window();
    window.type_keys("ix<Esc>");
    window.wait_title("con [+]", |title| title.contains("[+]"));
    window.request_close();
    window.type_keys(":w<CR>");
    smoke.wait_contents("a.md", "xuno\n");
    assert!(nota.is_running());
    window.request_close();
    assert!(nota.wait_exit().success());
    assert_eq!(smoke.files(), ["a.md"]);
}

#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn w_conserva_los_permisos() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    smoke.set_mode("a.md", 0o640);
    let mut nota = smoke.launch(&["a.md"]);
    let window = nota.window();
    window.type_keys("ix<Esc>:w<CR>");
    smoke.wait_contents("a.md", "xuno\n");
    assert_eq!(smoke.mode("a.md"), 0o640);
    window.type_keys(":q<CR>");
    assert!(nota.wait_exit().success());
    assert_eq!(smoke.files(), ["a.md"]);
}

#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn q_bang_sale_sin_guardar() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    let mut nota = smoke.launch(&["a.md"]);
    let window = nota.window();
    window.type_keys("ix<Esc>");
    window.wait_title("con [+]", |title| title.contains("[+]"));
    window.type_keys(":q!<CR>");
    assert!(nota.wait_exit().success());
    assert_eq!(smoke.read("a.md"), "uno\n");
    assert_eq!(smoke.files(), ["a.md"]);
}

/// `:e` cambia de archivo dentro de la misma ventana; cada uno se guarda aparte.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn e_edita_otro_archivo_en_la_misma_ventana() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    smoke.write("b.md", "dos\n");
    let mut nota = smoke.launch(&["a.md"]);
    let window = nota.window();
    window.type_keys("ix<Esc>:w<CR>:e b.md<CR>iy<Esc>:wq<CR>");
    assert!(nota.wait_exit().success());
    assert_eq!(smoke.read("a.md"), "xuno\n");
    assert_eq!(smoke.read("b.md"), "ydos\n");
    assert_eq!(smoke.files(), ["a.md", "b.md"]);
}
