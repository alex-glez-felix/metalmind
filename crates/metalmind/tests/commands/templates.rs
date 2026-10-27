//! `metalmind add` con plantilla (06 §11, docs/cli.md).

use pretty_assertions::assert_eq;
use serde_json::Value;

use super::{
    Sandbox, assert_json_error, assert_ok, is_string_array, json_line, run, stderr, stdout,
};

const ENTREVISTA: &str =
    "---\ntags: [entrevista]\n---\n# {{title}}\n\n## Resumen\n\n## Preguntas\n";

/// Un vault con plantillas y páginas de tag que las piden.
fn with_templates() -> Sandbox {
    let sandbox = Sandbox::new();
    for (path, text) in [
        ("plantillas/entrevista.md", ENTREVISTA),
        ("plantillas/semanal.md", "# Semana del {{date}}\n"),
        ("plantillas/rota.md", "# {{autor}}\n"),
        ("tags/reunion.md", "---\ntemplate: semanal\n---\n"),
    ] {
        let full = sandbox.vault.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }
    sandbox
}

#[test]
fn add_template_llena_la_plantilla_y_pone_el_texto_en_su_hueco() {
    let sandbox = with_templates();
    let output = run(
        &mut sandbox.metalmind(),
        &[
            "add",
            "--template",
            "entrevista",
            "--title",
            "Backend",
            "Sabe",
            "Rust",
        ],
    );
    assert_ok(&output);
    assert_eq!(stdout(&output), "backend.md\n");
    assert_eq!(
        sandbox.read("backend.md"),
        "---\ntags: [entrevista]\n---\n# Backend\n\n## Resumen\nSabe Rust\n\n## Preguntas\n"
    );
}

/// `-t` con un tag cuya página pide plantilla la usa; `--no-template`, no.
#[test]
fn un_tag_con_plantilla_la_usa_salvo_no_template() {
    let sandbox = with_templates();
    let output = run(
        &mut sandbox.metalmind(),
        &["add", "-t", "reunion", "acuerdos"],
    );
    assert_ok(&output);
    let text = sandbox.read(stdout(&output).trim_end());
    assert!(text.contains("# Semana del "), "{text}");
    assert!(text.ends_with("\nacuerdos\n"), "{text}");
    let output = run(
        &mut sandbox.metalmind(),
        &["add", "-t", "reunion", "--no-template", "otra", "cosa"],
    );
    assert_ok(&output);
    assert_eq!(
        sandbox.read("otra-cosa.md"),
        "---\ntags: [\"reunion\"]\n---\notra cosa\n"
    );
}

/// Lo que la plantilla no pudo llenar no impide crear: va en `problems` del
/// JSON o, sin `--json`, a stderr.
#[test]
fn los_problemas_de_la_plantilla_se_avisan_y_la_nota_se_crea() {
    let sandbox = with_templates();
    let output = run(
        &mut sandbox.metalmind(),
        &["add", "--json", "--template", "rota", "x"],
    );
    assert_ok(&output);
    let value = json_line(stdout(&output));
    assert_eq!(value["template"], "rota");
    assert!(is_string_array(&value["problems"]), "{value}");
    assert_eq!(
        value["problems"].as_array().map(Vec::len),
        Some(1),
        "{value}"
    );
    assert_eq!(value["path"], Value::from("x.md"));
    assert_eq!(sandbox.read("x.md"), "# {{autor}}\nx\n");
    let output = run(
        &mut sandbox.metalmind(),
        &["add", "--template", "rota", "y"],
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(stdout(&output), "y.md\n");
    assert_eq!(
        stderr(&output),
        "metalmind: y.md: en la plantilla: `{{autor}}` no es una variable de plantilla\n"
    );
}

#[test]
fn una_plantilla_que_no_existe_es_error_de_uso() {
    let sandbox = with_templates();
    let output = run(
        &mut sandbox.metalmind(),
        &["add", "--json", "--template", "nada", "x"],
    );
    assert_json_error(&output, "usage", 2);
    assert!(!sandbox.vault.join("x.md").exists());
}
