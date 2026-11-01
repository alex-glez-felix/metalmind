//! `metalmind tags` y la consulta de tags de `metalmind search` (05 §2–§4,
//! docs/cli.md): los mismos números y notas que el panel y `space f`.

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use super::{Sandbox, assert_json_error, assert_ok, assert_shape, json_line, run, stdout};

/// `#Rust` y `#rust` son un tag; `gpui` tiene a `rust` y a `ui` de padres, y
/// `rust` a `lenguajes`, que no tiene página ni nota.
fn with_tags() -> Sandbox {
    let sandbox = Sandbox::new();
    for (path, text) in [
        ("tags/rust.md", "---\nparents: [lenguajes]\n---\n"),
        ("tags/gpui.md", "---\nparents: [rust, ui]\n---\n"),
        ("parser.md", "#Rust y el parser"),
        ("proyectos/ventana.md", "---\ntags: [gpui]\n---\nUn parser"),
        ("viejo.md", "#Rust #archivado"),
        ("ideas/parser-de-rust.md", "#rust otra vez #Rust"),
        ("sin-tags.md", "Nada."),
        ("img/rust.png", ""),
    ] {
        let full = sandbox.vault.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }
    sandbox
}

fn search(sandbox: &Sandbox, query: &str) -> Vec<String> {
    let output = run(&mut sandbox.metalmind(), &["search", "--json", query]);
    assert_ok(&output);
    let value = json_line(stdout(&output));
    let mut paths: Vec<String> = value["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| hit["path"].as_str().unwrap().to_owned())
        .collect();
    paths.sort_unstable();
    paths
}

#[test]
fn tags_cuenta_como_el_panel_con_la_forma_mas_usada_y_los_padres() {
    let sandbox = with_tags();
    let output = run(&mut sandbox.metalmind(), &["tags"]);
    assert_ok(&output);
    assert_eq!(
        stdout(&output),
        "\
1  archivado
4  lenguajes
4  Rust  ← lenguajes
1  gpui  ← Rust, ui
1  ui
"
    );
}

#[test]
fn tags_json_tiene_la_forma_documentada() {
    let sandbox = with_tags();
    let output = run(&mut sandbox.metalmind(), &["tags", "--json"]);
    assert_ok(&output);
    let value = json_line(stdout(&output));
    assert_shape(
        &value,
        &[("vault", Value::is_string), ("tags", Value::is_array)],
    );
    for tag in value["tags"].as_array().unwrap() {
        assert_shape(
            tag,
            &[
                ("name", Value::is_string),
                ("count", Value::is_u64),
                ("parents", Value::is_array),
            ],
        );
    }
    assert_eq!(
        value["tags"][3],
        json!({"name": "gpui", "count": 1, "parents": ["Rust", "ui"]})
    );
}

#[test]
fn tags_de_un_vault_sin_tags_sale_vacio() {
    let sandbox = Sandbox::new();
    std::fs::write(sandbox.vault.join("nota.md"), "Sin tags.").unwrap();
    let output = run(&mut sandbox.metalmind(), &["tags"]);
    assert_ok(&output);
    assert_eq!(stdout(&output), "");
    let output = run(&mut sandbox.metalmind(), &["tags", "--json"]);
    assert_eq!(json_line(stdout(&output))["tags"], json!([]));
}

#[test]
fn search_con_tags_filtra_y_el_resto_va_al_fuzzy() {
    let sandbox = with_tags();
    let cases: [(&str, &[&str]); 7] = [
        (
            "#rust",
            &[
                "ideas/parser-de-rust.md",
                "parser.md",
                "proyectos/ventana.md",
                "viejo.md",
            ],
        ),
        (
            "#lenguajes -#archivado parser",
            &["ideas/parser-de-rust.md", "parser.md"],
        ),
        ("#RUST #ui", &["proyectos/ventana.md"]),
        ("-#archivado #gpui", &["proyectos/ventana.md"]),
        ("-#", &["sin-tags.md"]),
        ("#no-existe", &[]),
        ("-#no-existe rust", &["ideas/parser-de-rust.md"]),
    ];
    for (query, expected) in cases {
        assert_eq!(search(&sandbox, query), expected, "{query:?}");
    }
}

#[test]
fn search_sin_tags_sigue_buscando_rutas_con_adjuntos() {
    let sandbox = with_tags();
    assert_eq!(
        search(&sandbox, "rust"),
        ["ideas/parser-de-rust.md", "img/rust.png", "tags/rust.md"]
    );
}

#[test]
fn search_con_un_tag_invalido_es_error_de_uso() {
    let sandbox = with_tags();
    let output = run(
        &mut sandbox.metalmind(),
        &["search", "--json", "parser #2024/10"],
    );
    assert_json_error(&output, "usage", 2);
}
