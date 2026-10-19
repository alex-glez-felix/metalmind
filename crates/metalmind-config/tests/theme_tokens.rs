#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Los roles de color y las medidas de un tema: se heredan con `extends`
//! como los colores y nombran la paleta y la escala finales.

use std::fs;
use std::path::Path;

use metalmind_config::{Color, Length, Location, MarkColumns, Px, Theme, Themes};
use pretty_assertions::assert_eq;

fn load(text: &str) -> Result<Theme, metalmind_config::Diagnostic> {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), text);
    Themes::new(dir.path().to_owned())
        .load("a")
        .map(|loaded| loaded.value)
        .map_err(|failed| failed.error)
}

fn write(dir: &Path, text: &str) {
    fs::write(dir.join("a.yaml"), text).unwrap();
}

fn base() -> Theme {
    Themes::builtin().load("metalmind-oscuro").unwrap().value
}

#[test]
fn cambiar_un_rol_cambia_todo_lo_que_lo_nombra() {
    let theme = load("extends: metalmind-oscuro\nroles:\n  accent: \"#112233\"\n").unwrap();
    let accent = Color {
        rgb: 0x11_22_33,
        alpha: 1.,
    };
    assert_eq!(theme.roles.accent, accent);
    assert_eq!(theme.ui.picker_match, accent);
    assert_eq!(theme.ui.panel_folder, accent);
    assert_eq!(
        theme.ui.selection,
        Color {
            alpha: 0.3,
            ..accent
        }
    );
    assert_eq!(theme.ui.background, base().ui.background);
}

#[test]
fn un_rol_heredado_usa_la_paleta_final() {
    let theme = load("extends: metalmind-oscuro\npalette:\n  bg_dark: \"#000001\"\n").unwrap();
    assert_eq!(theme.roles.surface.rgb, 0x00_00_01);
    assert_eq!(theme.ui.panel_background.rgb, 0x00_00_01);
}

#[test]
fn la_paleta_no_puede_usar_el_nombre_de_un_rol() {
    let error = load("extends: metalmind-oscuro\npalette:\n  accent: \"#000001\"\n").unwrap_err();
    assert_eq!(
        error.message,
        "`accent` es un rol: la paleta no puede usar ese nombre"
    );
}

#[test]
fn una_medida_hereda_y_nombra_la_escala_final() {
    let theme =
        load("extends: metalmind-oscuro\nspacing:\n  md: 20\nlayout:\n  status_gap: md\n").unwrap();
    let layout = theme.metrics.layout;
    assert_eq!(
        layout.picker_padding_x,
        Length(20.),
        "heredada, con `md` de este tema"
    );
    assert_eq!(layout.status_gap, Length(20.));
    assert_eq!(
        layout.status_padding_x,
        base().metrics.layout.status_padding_x
    );
    assert_eq!(theme.metrics.text, base().metrics.text);
}

#[test]
fn un_paso_de_spacing_desconocido_es_error_con_posicion() {
    let error = load("extends: metalmind-oscuro\nlayout:\n  menu_padding: xxl\n").unwrap_err();
    assert_eq!(
        error.message,
        "`xxl` no es un paso de `spacing` (en layout.menu_padding): xs, sm, md, lg o píxeles"
    );
    assert_eq!(
        error.location,
        Some(Location {
            line: 3,
            column: 17
        })
    );
}

#[test]
fn medidas_fuera_de_rango_son_error() {
    let cases = [
        (
            "radius:\n  md: -1\n",
            "una medida en píxeles va de 0 a 2000",
        ),
        (
            "layout:\n  picker_width: 5000\n",
            "una medida en píxeles va de 0 a 2000",
        ),
        (
            "rows:\n  tree: 9\n",
            "un tamaño relativo a la fuente va de 0.5 a 4",
        ),
        (
            "text:\n  ui_md: { size: 1.0, line_height: 0.5 }\n",
            "un interlineado va de 1 a 3",
        ),
        ("text:\n  strong: 750\n", "un peso va de 100 a 900"),
        (
            "layout:\n  picker_rows: 0\n",
            "una cantidad de filas va de 1 a 100",
        ),
        (
            "layout:\n  gutter_mark_columns: 2\n",
            "las columnas de las marks van de 3 a 8",
        ),
        (
            "layout:\n  gutter_mark_columns: 9\n",
            "las columnas de las marks van de 3 a 8",
        ),
    ];
    for (yaml, expected) in cases {
        let error = load(&format!("extends: metalmind-oscuro\n{yaml}")).unwrap_err();
        assert!(
            error.message.contains(expected),
            "{yaml}: {}",
            error.message
        );
    }
}

/// El tema de ejemplo de `docs/config.md` carga y hace lo que dice.
#[test]
fn el_ejemplo_de_la_documentacion_carga() {
    let doc = include_str!("../../../docs/config.md");
    let start = doc.find("# ~/.config/metalmind/themes/mio.yaml").unwrap();
    let example = &doc[start..start + doc[start..].find("```").unwrap()];
    let theme = load(example).unwrap();
    assert_eq!(
        theme.roles.accent.rgb, 0x9e_ce_6a,
        "`green` de la paleta oscura"
    );
    assert_eq!(theme.ui.picker_match, theme.roles.accent);
    assert_eq!(theme.metrics.layout.picker_padding_x, Length(10.));
    assert_eq!(theme.metrics.radius.lg, Px(0.));
}

#[test]
fn un_rol_que_falta_se_nombra_como_rol() {
    let yaml = include_str!("../themes/metalmind-oscuro.yaml").replace("  accent: blue\n", "");
    let error = load(&yaml).unwrap_err();
    assert!(
        error
            .message
            .starts_with("el rol `accent` no está definido"),
        "{}",
        error.message
    );
}

#[test]
fn un_rol_no_nombra_otro_rol() {
    let error = load("extends: metalmind-oscuro\nroles:\n  surface: background\n").unwrap_err();
    assert_eq!(
        error.message,
        "`background` no está en la paleta (en roles.surface): un rol nombra la paleta o un hex"
    );
}

/// Con 3 columnas cabe la letra, el `+` y el espacio antes de los números.
#[test]
fn las_columnas_de_las_marks_aceptan_de_3_a_8() {
    for columns in [3, 8] {
        let theme = load(&format!(
            "extends: metalmind-oscuro\nlayout:\n  gutter_mark_columns: {columns}\n"
        ))
        .unwrap();
        assert_eq!(
            theme.metrics.layout.gutter_mark_columns,
            MarkColumns(columns)
        );
    }
}
