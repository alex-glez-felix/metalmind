//! Renombrar y fusionar tags con vista previa y deshacer (05 §5), en la app
//! real: lo que se ve en la vista previa no lo mira el smoke, sí lo que
//! queda en disco.

use super::support::Smoke;

/// `a.md` abierta, con un `#rust`, y `b.md` con otro.
fn two_notes(smoke: &Smoke) {
    smoke.write("a.md", "uno #rust\n");
    smoke.write("b.md", "#rust dos\n");
}

/// `enter` aplica la vista previa en las dos notas, también en la que está
/// abierta, y `:tagundo` las devuelve como estaban.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn renombrar_aplica_y_tagundo_lo_deshace() {
    let smoke = Smoke::new();
    two_notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys(":tagrename #rust #lang<CR><CR>");
    smoke.wait_contents("a.md", "uno #lang\n");
    smoke.wait_contents("b.md", "#lang dos\n");
    window.type_keys(":tagundo<CR>");
    smoke.wait_contents("a.md", "uno #rust\n");
    smoke.wait_contents("b.md", "#rust dos\n");
}

/// `x` quita la fila elegida: `j` baja a la de `b.md` y esa nota no cambia.
/// Fusionar en un tag que ya existe va por el mismo camino.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn x_quita_un_cambio_de_la_vista_previa() {
    let smoke = Smoke::new();
    two_notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    metalmind
        .window()
        .type_keys(":tagrename #rust #lang<CR>jx<CR>");
    smoke.wait_contents("a.md", "uno #lang\n");
    assert_eq!(smoke.read("b.md"), "#rust dos\n");
}

/// Con cambios sin guardar en `a.md`, `enter` no aplica nada; quitada su
/// fila, se aplica lo demás y el buffer queda como estaba.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn un_buffer_sin_guardar_bloquea_hasta_quitar_su_fila() {
    let smoke = Smoke::new();
    smoke.without_autosave();
    two_notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys("Ax<Esc>:tagrename #rust #lang<CR><CR>x<CR>");
    smoke.wait_contents("b.md", "#lang dos\n");
    assert_eq!(smoke.read("a.md"), "uno #rust\n");
    window.type_keys(":w<CR>");
    smoke.wait_contents("a.md", "uno #rustx\n");
}

/// `:tagmerge` pasa las notas de un tag a otro que ya existe, con su forma
/// establecida.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn fusionar_pasa_las_notas_al_otro_tag() {
    let smoke = Smoke::new();
    two_notes(&smoke);
    smoke.write("c.md", "#Lang y #Lang\n");
    let mut metalmind = smoke.launch(&["a.md"]);
    metalmind
        .window()
        .type_keys(":tagmerge #rust #lang<CR><CR>");
    smoke.wait_contents("a.md", "uno #Lang\n");
    smoke.wait_contents("b.md", "#Lang dos\n");
}

/// Desde la vista de tags: `r` sobre `#rust` pregunta el nombre nuevo y
/// `enter` lleva a la misma vista previa que `:tagrename`.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn r_en_la_vista_de_tags_renombra_el_tag() {
    let smoke = Smoke::new();
    two_notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    metalmind
        .window()
        .type_keys(" tggr<BS><BS><BS><BS>lang<CR><CR>");
    smoke.wait_contents("a.md", "uno #lang\n");
    smoke.wait_contents("b.md", "#lang dos\n");
}

/// `:tagundo` no escribe debajo de un buffer con cambios sin guardar; una
/// vez guardado, deshace lo demás y deja esa nota como quedó.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn tagundo_no_pisa_un_buffer_sin_guardar() {
    let smoke = Smoke::new();
    smoke.without_autosave();
    two_notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys(":tagrename #rust #lang<CR><CR>");
    smoke.wait_contents("a.md", "uno #lang\n");
    smoke.wait_contents("b.md", "#lang dos\n");
    window.type_keys("Gox<Esc>:tagundo<CR>:w<CR>");
    smoke.wait_contents("a.md", "uno #lang\nx\n");
    assert_eq!(smoke.read("b.md"), "#lang dos\n");
    window.type_keys(":tagundo<CR>");
    smoke.wait_contents("b.md", "#rust dos\n");
    assert_eq!(smoke.read("a.md"), "uno #lang\nx\n");
}

/// Con la vista previa bloqueada por un buffer sin guardar, el autoguardado
/// la desbloquea sin cerrarla.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn el_autoguardado_desbloquea_la_vista_previa() {
    let smoke = Smoke::new();
    two_notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys("Gox<Esc>:tagrename #rust #lang<CR>");
    smoke.wait_contents("a.md", "uno #rust\nx\n");
    window.type_keys("<CR>");
    smoke.wait_contents("a.md", "uno #lang\nx\n");
    smoke.wait_contents("b.md", "#lang dos\n");
}
