use pretty_assertions::assert_eq;

use super::*;
use crate::diagnostic::Severity;

const EVERY_SETTING: &str = "
theme: nota-claro
vault: /notas

font:
  family: Iosevka
  size: 13.5

editor:
  number: false
  relativenumber: false
  scrolloff: 5
  tabstop: 2
  expandtab: false
  padding:
    horizontal: 20
    vertical: 0

panes:
  border_width: 4

search:
  ignorecase: true
  smartcase: true
";

fn every_setting() -> Settings {
    Settings {
        theme: Spanned::new("nota-claro".to_owned()),
        vault: Some("/notas".into()),
        font: Font {
            family: Some("Iosevka".to_owned()),
            size: FontSize(13.5),
        },
        editor: Editor {
            number: false,
            relativenumber: false,
            scrolloff: 5,
            tabstop: TabStop(2),
            expandtab: false,
            padding: Padding {
                horizontal: 20,
                vertical: 0,
            },
        },
        panes: Panes { border_width: 4 },
        search: Search {
            ignorecase: true,
            smartcase: true,
        },
    }
}

#[test]
fn sin_archivo_todo_por_defecto() {
    let loaded = Settings::parse("");
    assert_eq!(loaded.value, Settings::default());
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value.theme(), "nota-oscuro");
    assert_eq!(loaded.value.font.size, FontSize(15.));
    assert_eq!(loaded.value.editor.tabstop.get(), 4);
    let editor = loaded.value.editor;
    assert!(
        editor.number && editor.relativenumber,
        "híbrido por defecto"
    );
}

#[test]
fn lee_cada_ajuste() {
    let loaded = Settings::parse(EVERY_SETTING);
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value, every_setting());
}

#[test]
fn la_misma_config_en_json_puro_carga_igual() {
    let json = r#"{
  "theme": "nota-claro",
  "vault": "/notas",
  "font": { "family": "Iosevka", "size": 13.5 },
  "editor": {
    "number": false,
    "relativenumber": false,
    "scrolloff": 5,
    "tabstop": 2,
    "expandtab": false,
    "padding": { "horizontal": 20, "vertical": 0 }
  },
  "panes": { "border_width": 4 },
  "search": { "ignorecase": true, "smartcase": true }
}"#;
    let loaded = Settings::parse(json);
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value, every_setting());
}

#[test]
fn un_tamano_entero_tambien_vale() {
    assert_eq!(
        Settings::parse("font:\n  size: 18\n").value.font.size,
        FontSize(18.)
    );
}

/// El problema de Noruega: YAML 1.1 lee `NO`, `on`, `yes` y `off` como
/// booleanos. Aquí son texto, o un error de tipo en un campo booleano.
#[test]
fn el_problema_de_noruega_no_existe() {
    assert_eq!(Settings::parse("theme: NO\n").value.theme(), "NO");
    assert_eq!(
        Settings::parse("font:\n  family: yes\n").value.font.family,
        Some("yes".to_owned())
    );
    for (word, column) in [("on", 14), ("yes", 14), ("off", 14), ("NO", 14)] {
        let loaded = Settings::parse(&format!("editor:\n  expandtab: {word}\n"));
        assert_eq!(loaded.value, Settings::default(), "{word}");
        let [error] = loaded.diagnostics.as_slice() else {
            panic!("{:?}", loaded.diagnostics)
        };
        assert_eq!(error.severity, Severity::Error);
        assert_eq!(error.location, Some(Location { line: 2, column }));
        assert!(error.message.contains("true or false"), "{}", error.message);
    }
}

#[test]
fn un_texto_que_parece_numero_queda_tal_cual() {
    for text in ["0755", "1.10", "0x1F", "1e3"] {
        let loaded = Settings::parse(&format!("font:\n  family: {text}\n"));
        assert_eq!(loaded.diagnostics, [], "{text}");
        assert_eq!(loaded.value.font.family.as_deref(), Some(text));
    }
}

#[test]
fn yaml_roto_usa_defaults_y_reporta_linea_y_columna() {
    let loaded = Settings::parse("theme: nota-claro\nfont: [size: 12\n");
    assert_eq!(loaded.value, Settings::default());
    let [error] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(error.severity, Severity::Error);
    assert!(error.location.is_some_and(|at| at.line >= 2), "{error:?}");
}

#[test]
fn indentacion_rota_usa_defaults_y_reporta_linea_y_columna() {
    let loaded = Settings::parse("editor:\n  scrolloff: 3\n tabstop: 2\n");
    assert_eq!(loaded.value, Settings::default());
    let [error] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(error.severity, Severity::Error);
    assert_eq!(error.location, Some(Location { line: 3, column: 2 }));
}

#[test]
fn tipo_equivocado_usa_defaults_y_reporta_posicion() {
    let loaded = Settings::parse("editor:\n  tabstop: cuatro\n");
    assert_eq!(loaded.value, Settings::default());
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(
        loaded.diagnostics[0].location,
        Some(Location {
            line: 2,
            column: 12
        })
    );
}

#[test]
fn valor_fuera_de_rango_es_error() {
    for (text, line) in [
        ("font:\n  size: 0", 2),
        ("font:\n  size: 900", 2),
        ("editor:\n  tabstop: 0", 2),
        ("editor:\n  padding:\n    vertical: 300", 3),
        ("editor:\n  padding:\n    horizontal: -1", 3),
    ] {
        let loaded = Settings::parse(text);
        assert_eq!(loaded.value, Settings::default(), "{text}");
        assert_eq!(
            loaded.diagnostics[0].location.map(|at| at.line),
            Some(line),
            "{text}"
        );
    }
}

#[test]
fn clave_desconocida_avisa_y_aplica_el_resto() {
    let loaded = Settings::parse("editor:\n  scrolloff: 3\n  scroloff: 4\n");
    assert_eq!(loaded.value.editor.scrolloff, 3);
    let [warning] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.location, Some(Location { line: 3, column: 3 }));
    assert!(warning.message.contains("editor.scroloff"));
}

#[test]
fn el_vault_expande_la_tilde() {
    let home = Path::new("/home/usuario");
    let vault = |text: &str| Settings::parse(text).value.vault(home);
    assert_eq!(vault(""), None);
    assert_eq!(vault("vault: ~"), None);
    assert_eq!(vault("vault: ~/Notas"), Some("/home/usuario/Notas".into()));
    assert_eq!(vault("vault: /srv/notas"), Some("/srv/notas".into()));
}
