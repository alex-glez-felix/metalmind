#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs;
use std::path::Path;

use metalmind_config::{BUILTIN_THEMES, Color, DEFAULT_THEME, Length, Location, Severity, Themes};
use pretty_assertions::assert_eq;

fn write(dir: &Path, name: &str, text: &str) {
    fs::write(dir.join(format!("{name}.yaml")), text).unwrap();
}

#[test]
fn los_temas_embebidos_cargan_sin_avisos() {
    for (name, _) in BUILTIN_THEMES {
        let loaded = Themes::builtin().load(name).unwrap();
        assert_eq!(loaded.diagnostics, [], "{name}");
        insta::assert_debug_snapshot!(*name, loaded.value);
    }
}

#[test]
fn el_tema_por_defecto_es_embebido() {
    assert!(
        BUILTIN_THEMES
            .iter()
            .any(|(name, _)| *name == DEFAULT_THEME)
    );
}

#[test]
fn un_tema_del_usuario_extiende_y_cambia_la_paleta_heredada() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "mio",
        "extends: metalmind-oscuro\npalette:\n  blue: \"#112233\"\nui:\n  error: \"#ff0000\"\n",
    );
    let theme = Themes::new(dir.path().to_owned())
        .load("mio")
        .unwrap()
        .value;
    let base = Themes::builtin().load("metalmind-oscuro").unwrap().value;
    assert_eq!(theme.name, "mio");
    assert_eq!(
        theme.ui.error,
        Color {
            rgb: 0xff_00_00,
            alpha: 1.
        }
    );
    assert_eq!(
        theme.cursor.insert,
        Color {
            rgb: 0x11_22_33,
            alpha: 1.
        }
    );
    assert_eq!(
        theme.ui.selection,
        Color {
            rgb: 0x11_22_33,
            alpha: 0.3
        }
    );
    assert_eq!(theme.ui.background, base.ui.background);
    assert_eq!(theme.syntax.list_marker, theme.cursor.insert);
    assert_eq!(theme.syntax.bold, base.syntax.bold);
}

#[test]
fn extends_encadenado_entre_temas_del_usuario() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends: b\nui:\n  error: \"#000001\"\n");
    write(
        dir.path(),
        "b",
        "extends: metalmind-claro\nui:\n  error: \"#000002\"\n  modified: \"#000003\"\n",
    );
    let theme = Themes::new(dir.path().to_owned()).load("a").unwrap().value;
    assert_eq!(theme.ui.error.rgb, 1);
    assert_eq!(theme.ui.modified.rgb, 3);
}

#[test]
fn un_tema_del_usuario_reemplaza_al_embebido_del_mismo_nombre() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "base", "extends: metalmind-oscuro\n");
    write(
        dir.path(),
        "metalmind-claro",
        "extends: base\nui:\n  error: \"#000009\"\n",
    );
    let theme = Themes::new(dir.path().to_owned())
        .load("metalmind-claro")
        .unwrap()
        .value;
    assert_eq!(theme.ui.error.rgb, 9);
}

#[test]
fn un_tema_del_usuario_cambia_el_ancho_del_menu() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "mio",
        "extends: metalmind-oscuro\nlayout:\n  menu_width: 300\n",
    );
    let loaded = Themes::new(dir.path().to_owned()).load("mio").unwrap();
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value.metrics.layout.menu_width, Length(300.));
}

#[test]
fn un_tema_del_usuario_puede_extender_al_embebido_que_reemplaza() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "metalmind-oscuro",
        "extends: metalmind-oscuro\nui:\n  error: \"#000009\"\n",
    );
    let loaded = Themes::new(dir.path().to_owned())
        .load("metalmind-oscuro")
        .unwrap();
    let base = Themes::builtin().load("metalmind-oscuro").unwrap().value;
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value.ui.error.rgb, 9);
    assert_eq!(loaded.value.ui.background, base.ui.background);
}

#[test]
fn un_ciclo_que_pasa_por_el_reemplazo_de_un_embebido_es_circular() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "metalmind-oscuro", "extends: x\n");
    write(dir.path(), "x", "extends: x\n");
    let error = Themes::new(dir.path().to_owned())
        .load("metalmind-oscuro")
        .unwrap_err()
        .error;
    assert_eq!(
        error.message,
        "`extends` circular: metalmind-oscuro → x → x"
    );
}

#[test]
fn volver_al_reemplazo_de_un_embebido_por_otro_tema_es_circular() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "metalmind-oscuro", "extends: x\n");
    write(dir.path(), "x", "extends: metalmind-oscuro\n");
    let error = Themes::new(dir.path().to_owned())
        .load("metalmind-oscuro")
        .unwrap_err()
        .error;
    assert_eq!(
        error.message,
        "`extends` circular: metalmind-oscuro → x → metalmind-oscuro"
    );
    assert_eq!(error.file, Some(dir.path().join("x.yaml")));
}

#[test]
fn extends_circular_es_error_con_la_cadena() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends: b\n");
    write(dir.path(), "b", "# b\nextends: a\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(error.message, "`extends` circular: a → b → a");
    assert_eq!(error.file, Some(dir.path().join("b.yaml")));
    assert_eq!(
        error.location,
        Some(Location {
            line: 2,
            column: 10
        })
    );
}

#[test]
fn un_tema_que_se_extiende_a_si_mismo_es_circular() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends: a\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(error.message, "`extends` circular: a → a");
}

#[test]
fn extends_a_un_tema_que_no_existe() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends: nada\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(error.message, "no existe el tema `nada`");
    assert_eq!(error.file, Some(dir.path().join("a.yaml")));
    assert_eq!(
        error.location,
        Some(Location {
            line: 1,
            column: 10
        })
    );
}

#[test]
fn sin_extends_le_faltan_colores() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "ui:\n  background: \"#000000\"\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert!(
        error
            .message
            .starts_with("al tema `a` le faltan claves: roles.background, roles.surface"),
        "{}",
        error.message
    );
    for key in [
        "ui.foreground, ui.selection",
        "cursor.normal",
        "syntax.markup",
        "headings.h6",
        "spacing.xs",
        "text.editor",
        "rows.tree",
    ] {
        assert!(error.message.contains(key), "{key}: {}", error.message);
    }
    assert!(
        error.message.ends_with("layout.math_block_margin"),
        "{}",
        error.message
    );
}

#[test]
fn un_tema_cambia_un_nivel_de_heading_y_hereda_los_demas() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "mio",
        "extends: metalmind-oscuro\nheadings:\n  h2: { size: 2.0, weight: 400 }\n",
    );
    let theme = Themes::new(dir.path().to_owned())
        .load("mio")
        .unwrap()
        .value;
    let base = Themes::builtin().load("metalmind-oscuro").unwrap().value;
    assert_eq!(
        theme.headings.0[1],
        metalmind_config::Heading {
            size: 2.0,
            weight: 400
        }
    );
    assert_eq!(theme.headings.0[0], base.headings.0[0]);
}

#[test]
fn un_heading_fuera_de_rango_es_error_con_posicion() {
    let dir = tempfile::tempdir().unwrap();
    for (text, wanted) in [
        (
            "headings:\n  h1: { size: 9, weight: 700 }\n",
            "tamaño de un heading",
        ),
        (
            "headings:\n  h1: { size: 1.5, weight: 750 }\n",
            "peso de un heading",
        ),
    ] {
        write(
            dir.path(),
            "a",
            &format!("extends: metalmind-oscuro\n{text}"),
        );
        let error = Themes::new(dir.path().to_owned())
            .load("a")
            .unwrap_err()
            .error;
        assert!(error.message.contains(wanted), "{}", error.message);
        assert_eq!(
            error.location.map(|at| at.line),
            Some(3),
            "{}",
            error.message
        );
    }
}

#[test]
fn nombre_de_paleta_desconocido_con_posicion() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a",
        "extends: metalmind-oscuro\ncursor:\n  insert: { color: azull }\n",
    );
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(
        error.message,
        "`azull` no está en la paleta ni es un rol (en cursor.insert)"
    );
    assert_eq!(
        error.location,
        Some(Location {
            line: 3,
            column: 11
        })
    );
}

#[test]
fn yaml_roto_y_tipo_equivocado_en_un_tema() {
    let dir = tempfile::tempdir().unwrap();
    let themes = Themes::new(dir.path().to_owned());
    write(dir.path(), "roto", "ui:\n  error: red\n modified: red\n");
    let error = themes.load("roto").unwrap_err().error;
    assert_eq!(error.file, Some(dir.path().join("roto.yaml")));
    assert_eq!(error.location, Some(Location { line: 3, column: 2 }));
    write(
        dir.path(),
        "tipo",
        "extends: metalmind-oscuro\npalette:\n  blue: 3\n",
    );
    let error = themes.load("tipo").unwrap_err().error;
    assert_eq!(error.location, Some(Location { line: 3, column: 9 }));
}

#[test]
fn clave_desconocida_en_un_tema_avisa_y_lo_aplica() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a",
        "extends: metalmind-oscuro\nui:\n  fondo: \"#000000\"\n",
    );
    let loaded = Themes::new(dir.path().to_owned()).load("a").unwrap();
    let [warning] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.file, Some(dir.path().join("a.yaml")));
    assert_eq!(warning.location, Some(Location { line: 3, column: 3 }));
}

#[test]
fn clave_de_mas_en_un_color_con_alpha_avisa_y_lo_aplica() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a",
        "extends: metalmind-oscuro\nui:\n  selection: { color: \"#010203\", alhpa: 0.5 }\n",
    );
    let loaded = Themes::new(dir.path().to_owned()).load("a").unwrap();
    assert_eq!(
        loaded.value.ui.selection,
        Color {
            rgb: 0x01_02_03,
            alpha: 1.
        }
    );
    let [warning] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(warning.severity, Severity::Warning);
    assert!(
        warning.message.contains("ui.selection.alhpa"),
        "{}",
        warning.message
    );
    assert_eq!(
        warning.location,
        Some(Location {
            line: 3,
            column: 34
        })
    );
}

#[test]
fn un_tema_que_falla_conserva_sus_avisos() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends: b\nui:\n  fondo: \"#000000\"\n");
    write(dir.path(), "b", "ui:\n  selction: \"#000000\"\n");
    let failed = Themes::new(dir.path().to_owned()).load("a").unwrap_err();
    assert!(
        failed.error.message.contains("ui.selection"),
        "{}",
        failed.error.message
    );
    let keys: Vec<_> = failed
        .warnings
        .iter()
        .map(|warning| (warning.file.clone(), warning.location))
        .collect();
    assert_eq!(
        keys,
        [
            (
                Some(dir.path().join("a.yaml")),
                Some(Location { line: 3, column: 3 })
            ),
            (
                Some(dir.path().join("b.yaml")),
                Some(Location { line: 2, column: 3 })
            ),
        ]
    );
}

#[test]
fn nombres_que_no_son_un_archivo_del_directorio() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["", "../fuera", "a/b", ".oculto"] {
        let error = Themes::new(dir.path().to_owned())
            .load(name)
            .unwrap_err()
            .error;
        assert!(
            error.message.contains("no es un nombre de tema válido"),
            "{name}"
        );
    }
}

#[test]
fn un_color_nulo_es_error_y_no_hereda() {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in [
        (
            "nulo",
            "extends: metalmind-oscuro\nui:\n  background: null\n",
        ),
        ("vacio", "extends: metalmind-oscuro\nui:\n  background:\n"),
    ] {
        write(dir.path(), name, text);
        let error = Themes::new(dir.path().to_owned())
            .load(name)
            .unwrap_err()
            .error;
        assert_eq!(error.severity, Severity::Error, "{name}");
        assert_eq!(error.location.map(|at| at.line), Some(3), "{name}: {error}");
    }
}
