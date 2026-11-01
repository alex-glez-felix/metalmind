//! La línea de `:` como pop-up centrado arriba (#193): su lista ofrece el
//! historial y los nombres de comando, y `tab` escribe la fila elegida.

use super::support::{Area, Smoke, Window};

/// Bajo lo escrito en el pop-up, centrado arriba como la paleta: con la
/// config por defecto, mide 640 y empieza a 64 del borde de arriba.
pub fn popup_list(window: &Window<'_>) -> Area {
    let (width, _, _) = window.pixels();
    Area {
        x: i16::try_from(width / 2).unwrap() - 320,
        y: 100,
        width: 640,
        height: 220,
    }
}

/// Con la config por defecto, las filas del pop-up miden 33 (su línea de
/// 21 y 6 arriba y abajo) y la primera empieza bajo el input, a 110.
const ROW_HEIGHT: i16 = 33;
const FIRST_ROW: i16 = 110;

/// `:foc` ofrece `focus` y `tab` lo escribe con su espacio: lo que sigue ya
/// es el argumento.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn tab_completa_el_nombre_del_comando() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    smoke.write("b.md", "#rust dos\n");
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    let list = popup_list(&window);
    let closed = window.wait_still(list);
    window.type_keys(":foc");
    window.wait_change("la lista de nombres", list, &closed);
    window.wait_still(list);
    window.type_keys("<Tab>#rust<CR><Space>f<CR>");
    window.wait_title("de b.md", |title| title.starts_with("b.md"));
}

/// Con la línea vacía, la lista es el historial y `tab` escribe lo último
/// que se envió; las flechas lo siguen recorriendo, como en vim.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn el_historial_se_completa_con_tab_y_se_recorre_con_las_flechas() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    smoke.write("b.md", "dos\n");
    smoke.write("c.md", "tres\n");
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys(":e b.md<CR>");
    window.wait_title("de b.md", |title| title.starts_with("b.md"));
    window.type_keys(":e c.md<CR>");
    window.wait_title("de c.md", |title| title.starts_with("c.md"));
    let list = popup_list(&window);
    let closed = window.wait_still(list);
    window.type_keys(":");
    window.wait_change("la lista del historial", list, &closed);
    window.wait_still(list);
    window.type_keys("<Tab><BS><BS><BS><BS>b.md<CR>");
    window.wait_title("de b.md otra vez", |title| title.starts_with("b.md"));
    window.type_keys(":<Up><Up><CR>");
    window.wait_title("de c.md otra vez", |title| title.starts_with("c.md"));
}

/// `:e` completa los archivos del vault con sus espacios escapados (#45):
/// `tab` escribe la primera fila y un clic, la que se toca; `enter` abre lo
/// escrito.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn e_completa_archivos_con_tab_y_con_un_clic() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    smoke.write("Mis notas/d.md", "cuatro\n");
    smoke.write("Mis notas/e.md", "cinco\n");
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    let list = popup_list(&window);
    let closed = window.wait_still(list);
    window.type_keys(r":e Mis\ notas");
    window.wait_change("la lista de archivos", list, &closed);
    window.wait_still(list);
    window.type_keys("<Tab><CR>");
    window.wait_title("de Mis notas/d.md", |title| {
        title.starts_with("Mis notas/d.md")
    });
    window.wait_still(list);
    window.type_keys(r":e Mis\ notas");
    window.wait_change("la lista de archivos otra vez", list, &closed);
    window.wait_still(list);
    let second_row = FIRST_ROW + ROW_HEIGHT + ROW_HEIGHT / 2;
    window.click(list.x + i16::try_from(list.width / 2).unwrap(), second_row);
    window.type_keys("<CR>");
    window.wait_title("de Mis notas/e.md", |title| {
        title.starts_with("Mis notas/e.md")
    });
}
