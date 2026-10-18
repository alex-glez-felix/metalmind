#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs;
use std::path::Path;

use nota_config::{BUILTIN_THEMES, Color, DEFAULT_THEME, Location, Severity, Themes};
use pretty_assertions::assert_eq;

fn write(dir: &Path, name: &str, text: &str) {
    fs::write(dir.join(format!("{name}.toml")), text).unwrap();
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
        "extends = \"nota-oscuro\"\n[palette]\nblue = \"#112233\"\n[ui]\nerror = \"#ff0000\"\n",
    );
    let theme = Themes::new(dir.path().to_owned())
        .load("mio")
        .unwrap()
        .value;
    let base = Themes::builtin().load("nota-oscuro").unwrap().value;
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
    assert_eq!(theme.syntax.heading, theme.cursor.insert);
    assert_eq!(theme.syntax.bold, base.syntax.bold);
}

#[test]
fn extends_encadenado_entre_temas_del_usuario() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a",
        "extends = \"b\"\n[ui]\nerror = \"#000001\"\n",
    );
    write(
        dir.path(),
        "b",
        "extends = \"nota-claro\"\n[ui]\nerror = \"#000002\"\nmodified = \"#000003\"\n",
    );
    let theme = Themes::new(dir.path().to_owned()).load("a").unwrap().value;
    assert_eq!(theme.ui.error.rgb, 1);
    assert_eq!(theme.ui.modified.rgb, 3);
}

#[test]
fn un_tema_del_usuario_reemplaza_al_embebido_del_mismo_nombre() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "base", "extends = \"nota-oscuro\"\n");
    write(
        dir.path(),
        "nota-claro",
        "extends = \"base\"\n[ui]\nerror = \"#000009\"\n",
    );
    let theme = Themes::new(dir.path().to_owned())
        .load("nota-claro")
        .unwrap()
        .value;
    assert_eq!(theme.ui.error.rgb, 9);
}

#[test]
fn un_tema_del_usuario_puede_extender_al_embebido_que_reemplaza() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "nota-oscuro",
        "extends = \"nota-oscuro\"\n[ui]\nerror = \"#000009\"\n",
    );
    let loaded = Themes::new(dir.path().to_owned())
        .load("nota-oscuro")
        .unwrap();
    let base = Themes::builtin().load("nota-oscuro").unwrap().value;
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value.ui.error.rgb, 9);
    assert_eq!(loaded.value.ui.background, base.ui.background);
}

#[test]
fn un_ciclo_que_pasa_por_el_reemplazo_de_un_embebido_es_circular() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "nota-oscuro", "extends = \"x\"\n");
    write(dir.path(), "x", "extends = \"x\"\n");
    let error = Themes::new(dir.path().to_owned())
        .load("nota-oscuro")
        .unwrap_err()
        .error;
    assert_eq!(error.message, "`extends` circular: nota-oscuro → x → x");
}

#[test]
fn extends_circular_es_error_con_la_cadena() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends = \"b\"\n");
    write(dir.path(), "b", "# b\nextends = \"a\"\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(error.message, "`extends` circular: a → b → a");
    assert_eq!(error.file, Some(dir.path().join("b.toml")));
    assert_eq!(
        error.location,
        Some(Location {
            line: 2,
            column: 11
        })
    );
}

#[test]
fn un_tema_que_se_extiende_a_si_mismo_es_circular() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends = \"a\"\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(error.message, "`extends` circular: a → a");
}

#[test]
fn extends_a_un_tema_que_no_existe() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "extends = \"nada\"\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(error.message, "no existe el tema `nada`");
    assert_eq!(error.file, Some(dir.path().join("a.toml")));
    assert_eq!(
        error.location,
        Some(Location {
            line: 1,
            column: 11
        })
    );
}

#[test]
fn sin_extends_le_faltan_colores() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a", "[ui]\nbackground = \"#000000\"\n");
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert!(
        error
            .message
            .starts_with("al tema `a` le faltan colores: ui.foreground, ui.selection"),
        "{}",
        error.message
    );
    assert!(error.message.contains("cursor.normal") && error.message.contains("syntax.markup"));
}

#[test]
fn nombre_de_paleta_desconocido_con_posicion() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a",
        "extends = \"nota-oscuro\"\n[cursor]\ninsert = { color = \"azull\" }\n",
    );
    let error = Themes::new(dir.path().to_owned())
        .load("a")
        .unwrap_err()
        .error;
    assert_eq!(
        error.message,
        "`azull` no está en la paleta (en cursor.insert)"
    );
    assert_eq!(
        error.location,
        Some(Location {
            line: 3,
            column: 10
        })
    );
}

#[test]
fn toml_roto_y_tipo_equivocado_en_un_tema() {
    let dir = tempfile::tempdir().unwrap();
    let themes = Themes::new(dir.path().to_owned());
    write(dir.path(), "roto", "[ui\n");
    let error = themes.load("roto").unwrap_err().error;
    assert_eq!(error.file, Some(dir.path().join("roto.toml")));
    assert_eq!(error.location.map(|at| at.line), Some(1));
    write(
        dir.path(),
        "tipo",
        "extends = \"nota-oscuro\"\n[palette]\nblue = 3\n",
    );
    let error = themes.load("tipo").unwrap_err().error;
    assert_eq!(error.location, Some(Location { line: 3, column: 8 }));
}

#[test]
fn clave_desconocida_en_un_tema_avisa_y_lo_aplica() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a",
        "extends = \"nota-oscuro\"\n[ui]\nfondo = \"#000000\"\n",
    );
    let loaded = Themes::new(dir.path().to_owned()).load("a").unwrap();
    let [warning] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.file, Some(dir.path().join("a.toml")));
    assert_eq!(warning.location, Some(Location { line: 3, column: 1 }));
}

#[test]
fn clave_de_mas_en_un_color_con_alpha_avisa_y_lo_aplica() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a",
        "extends = \"nota-oscuro\"\n[ui]\nselection = { color = \"#010203\", alhpa = 0.5 }\n",
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
    write(
        dir.path(),
        "a",
        "extends = \"b\"\n[ui]\nfondo = \"#000000\"\n",
    );
    write(dir.path(), "b", "[ui]\nselction = \"#000000\"\n");
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
                Some(dir.path().join("a.toml")),
                Some(Location { line: 3, column: 1 })
            ),
            (
                Some(dir.path().join("b.toml")),
                Some(Location { line: 2, column: 1 })
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
