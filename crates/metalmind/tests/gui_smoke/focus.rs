//! El modo focus (05 §3) en la app real: con focus, el finder ofrece solo
//! las notas del alcance.

use super::support::Smoke;

/// `a.md` abierta y sin tags, `b.md` sin tags y `c.md` con `#rust`: sin
/// focus, el primero del finder es otra nota que `c.md`.
fn notes(smoke: &Smoke) {
    smoke.write("a.md", "uno\n");
    smoke.write("b.md", "dos\n");
    smoke.write("c.md", "#rust tres\n");
}

#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn con_focus_el_finder_solo_ofrece_lo_del_alcance() {
    let smoke = Smoke::new();
    notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys(":focus #rust<CR><Space>f<CR>");
    window.wait_title("de c.md", |title| title.starts_with("c.md"));
    window.type_keys(":unfocus<CR><Space>fb.md<CR>");
    window.wait_title("de b.md", |title| title.starts_with("b.md"));
}

/// `F` sobre un tag de la vista de tags lo pone como focus.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn f_en_el_panel_enfoca_el_tag_elegido() {
    let smoke = Smoke::new();
    notes(&smoke);
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys("<Space>tggF<C-w>l<Space>f<CR>");
    window.wait_title("de c.md", |title| title.starts_with("c.md"));
}
