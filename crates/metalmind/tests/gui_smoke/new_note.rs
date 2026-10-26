//! Notas nuevas desde plantillas y la nota del día (06 §11), en la app real:
//! lo que queda en disco y dónde quedó el cursor, que se ve por dónde cae lo
//! que se teclea.

use std::fs;

use super::support::Smoke;

fn template(smoke: &Smoke, name: &str, text: &str) {
    let dir = smoke.notes().join("plantillas");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{name}.md")), text).unwrap();
}

const ENTREVISTA: &str =
    "---\ntags: [entrevista]\n---\n# {{title}}\n\n## Resumen\n\n## Preguntas\n";

/// `space n` elige la plantilla por parte de su nombre, `enter` pregunta el
/// nombre y otro `enter` la crea con sus carpetas y la abre con el cursor
/// bajo `## Resumen`.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn space_n_crea_la_nota_desde_la_plantilla() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    template(&smoke, "entrevista", ENTREVISTA);
    template(&smoke, "diario", "# {{date}}\n");
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys("<Space>nentre<CR>entrevistas/Backend senior<CR>");
    window.wait_title("de la nota nueva", |title| {
        title.starts_with("entrevistas/Backend senior.md")
    });
    window.type_keys("ilisto<Esc>:wq<CR>");
    assert!(metalmind.wait_exit().success());
    assert_eq!(
        smoke.read("entrevistas/Backend senior.md"),
        "---\ntags: [entrevista]\n---\n# Backend senior\n\n## Resumen\nlisto\n## Preguntas\n"
    );
    assert_eq!(smoke.read("a.md"), "uno\n");
}

/// Un nombre tomado no cierra la pregunta ni pisa la nota: se corrige y se
/// crea con el nombre nuevo.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn un_nombre_tomado_no_pisa_y_se_corrige() {
    let smoke = Smoke::new();
    smoke.write("a.md", "uno\n");
    template(&smoke, "entrevista", ENTREVISTA);
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys(":template entrevista<CR>a<CR>2<CR>");
    window.wait_title("de la nota nueva", |title| title.starts_with("a2.md"));
    window.type_keys(":wq<CR>");
    assert!(metalmind.wait_exit().success());
    assert_eq!(smoke.read("a.md"), "uno\n");
    assert!(
        smoke
            .read("a2.md")
            .starts_with("---\ntags: [entrevista]\n---\n# a2\n")
    );
}

/// `:today` crea la nota del día con su plantilla en su carpeta, y la
/// segunda vez la abre sin tocarla.
#[test]
#[ignore = "necesita Xvfb y Vulkan: ver docs/testing.md"]
fn today_crea_la_nota_del_dia_y_despues_la_abre() {
    let smoke = Smoke::new();
    smoke.config("daily:\n  enabled: true\n  format: '[hoy]'\n  template: diario\n");
    smoke.write("a.md", "uno\n");
    template(&smoke, "diario", "# Hoy\n\n");
    let mut metalmind = smoke.launch(&["a.md"]);
    let window = metalmind.window();
    window.type_keys(":today<CR>");
    window.wait_title("de la nota del día", |title| {
        title.starts_with("diario/hoy.md")
    });
    window.type_keys("iescrito<Esc>:w<CR>:e a.md<CR>");
    window.wait_title("de a.md", |title| title.starts_with("a.md"));
    window.type_keys("<Space>d");
    window.wait_title("de la nota del día", |title| {
        title.starts_with("diario/hoy.md")
    });
    window.type_keys(":wq<CR>");
    assert!(metalmind.wait_exit().success());
    assert_eq!(smoke.read("diario/hoy.md"), "# Hoy\nescrito\n");
}
