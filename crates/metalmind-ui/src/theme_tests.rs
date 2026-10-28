use gpui_kit::{Hsla, rgb};

use super::*;

fn installed(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

#[test]
fn elige_la_primera_candidata_instalada() {
    let fonts = installed(&["Cantarell", "Liberation Mono", "DejaVu Sans Mono"]);
    assert_eq!(
        first_installed(MONO_FONTS, &fonts).as_deref(),
        Some("DejaVu Sans Mono")
    );
}

#[test]
fn la_fuente_de_la_config_si_esta_instalada() {
    let fonts = installed(&["Iosevka", "DejaVu Sans Mono"]);
    assert_eq!(pick_font(Some("Iosevka"), &fonts).unwrap(), "Iosevka");
    assert_eq!(
        pick_font(Some("No Existe"), &fonts).unwrap(),
        "DejaVu Sans Mono"
    );
    assert_eq!(pick_font(None, &fonts).unwrap(), "DejaVu Sans Mono");
}

/// Los valores que `metalmind-ui` tenía fijos antes de leer temas, y los de paneles.
/// El borde entre paneles es `comment`, con contraste 3:1: con `border` no se veía.
#[test]
fn el_tema_oscuro_embebido_se_ve_igual_que_antes() {
    let theme = Theme::with_font(&config::Config::defaults().unwrap(), "Mono".into());
    let blue = Hsla::from(rgb(0x7a_a2_f7));
    let expected: [(&str, Hsla, Hsla); 16] = [
        ("background", theme.ui.background, rgb(0x1c_1f_24).into()),
        ("foreground", theme.ui.foreground, rgb(0xd7_da_e0).into()),
        ("caret", theme.cursor.insert, rgb(0x7a_a2_f7).into()),
        ("block_cursor", theme.cursor.normal, blue.opacity(0.5)),
        (
            "visual_cursor",
            theme.cursor.visual,
            Hsla::from(rgb(0xbb_9a_f7)).opacity(0.5),
        ),
        (
            "unfocused_cursor",
            theme.cursor.unfocused,
            Hsla::from(rgb(0x9a_a0_a6)).opacity(0.35),
        ),
        ("selection", theme.ui.selection, blue.opacity(0.3)),
        (
            "search",
            theme.ui.search,
            Hsla::from(rgb(0xe0_af_68)).opacity(0.3),
        ),
        (
            "search_current",
            theme.ui.search_current,
            Hsla::from(rgb(0x00ff_9e64)).opacity(0.6),
        ),
        (
            "status_background",
            theme.ui.status_background,
            rgb(0x16_18_1d).into(),
        ),
        (
            "status_foreground",
            theme.ui.status_foreground,
            rgb(0x9a_a0_a6).into(),
        ),
        (
            "status_unfocused_foreground",
            theme.ui.status_unfocused_foreground,
            rgb(0x5c_63_70).into(),
        ),
        (
            "status_border",
            theme.ui.status_border,
            rgb(0x2a_2e_36).into(),
        ),
        ("pane_border", theme.ui.pane_border, rgb(0x5c_63_70).into()),
        ("modified", theme.ui.modified, rgb(0xe0_af_68).into()),
        ("error", theme.ui.error, rgb(0xf7_76_8e).into()),
    ];
    for (name, actual, before) in expected {
        assert_eq!(actual, before, "{name}");
    }
    assert_eq!(theme.ui_font_size, px(15.));
    assert_eq!(theme.editor_font_size, px(15.));
}

#[test]
fn la_parte_del_detalle_del_picker_sale_del_tema() {
    let theme = Theme::with_font(&config::Config::defaults().unwrap(), "Mono".into());
    assert!((theme.layout.picker_detail_share - 0.4).abs() < 1e-6);
}

/// Las medidas que la UI tenía fijas antes de leerlas del tema, con la fuente de 15.
#[test]
fn las_medidas_de_fabrica_son_las_de_antes() {
    let theme = Theme::with_font(&config::Config::defaults().unwrap(), "Mono".into());
    let expected = [
        ("status_padding_x", theme.layout.status_padding_x, 10.),
        ("status_padding_y", theme.layout.status_padding_y, 3.),
        ("status_gap", theme.layout.status_gap, 16.),
        ("picker_width", theme.layout.picker_width, 640.),
        ("picker_top", theme.layout.picker_top, 64.),
        ("picker_padding_x", theme.layout.picker_padding_x, 12.),
        ("picker_row_height", theme.picker_row_height(), 33.),
        (
            "picker_input_padding_y",
            theme.layout.picker_input_padding_y,
            12.,
        ),
        ("picker_gap", theme.layout.picker_gap, 16.),
        ("panel_row_height", theme.panel_row_height, 26.),
        ("menu_row_height", theme.menu_row_height, 26.),
        ("panel_indent", theme.layout.panel_indent, 14.),
        ("panel_padding_x", theme.layout.panel_padding_x, 8.),
        ("panel_padding_y", theme.layout.panel_padding_y, 6.),
        ("panel_gap", theme.layout.panel_gap, 6.),
        (
            "panel_button_padding_x",
            theme.layout.panel_button_padding_x,
            4.,
        ),
        ("icon_size", theme.icon_size, 12.),
        ("icon_short_side", theme.icon_short_side(), 9.),
        ("rail_width", theme.layout.rail_width, 36.),
        ("rail_button_size", theme.layout.rail_button_size, 28.),
        ("list_width", theme.layout.list_width, 360.),
        (
            "list_open_min_width",
            theme.layout.list_open_min_width,
            1600.,
        ),
        ("list_row_height", theme.list_row_height(), 105.),
        ("radius_sm", theme.radius.sm, 2.),
        ("radius_md", theme.radius.md, 4.),
        ("radius_lg", theme.radius.lg, 8.),
        ("menu_width", theme.layout.menu_width, 240.),
        ("menu_padding", theme.layout.menu_padding, 4.),
        ("menu_item_padding_x", theme.layout.menu_item_padding_x, 8.),
        ("caret_width", theme.stroke.caret, 2.),
        ("border_width", theme.stroke.border, 1.),
        ("underline_thickness", theme.stroke.text_line, 1.),
        ("strike_thickness", theme.stroke.text_line, 1.),
        ("ui_small_font_size", theme.ui_small_font_size, 15.),
    ];
    for (name, actual, before) in expected {
        assert_eq!(actual, px(before), "{name}");
    }
    assert_eq!(theme.layout.picker_rows, 12);
    assert_eq!(
        (theme.text.editor.line_height, theme.text.ui_md.line_height),
        (1.4, 1.4)
    );
    assert_eq!(
        (theme.text.normal, theme.text.strong),
        (FontWeight::NORMAL, FontWeight::BOLD)
    );
}

#[test]
fn las_medidas_salen_del_tema() {
    let mut config = config::Config::defaults().unwrap();
    let metrics = &mut config.theme.metrics;
    metrics.layout.picker_padding_x = config::Length(20.);
    metrics.rows.tree = config::Factor(2.);
    metrics.radius.lg = config::Px(0.);
    metrics.text.ui_md.size = 0.8;
    metrics.text.strong = config::Weight(600);
    let theme = Theme::with_font(&config, "Mono".into());
    assert_eq!(theme.layout.picker_padding_x, px(20.));
    assert_eq!(theme.ui_font_size, px(12.));
    assert_eq!(
        theme.panel_row_height,
        px(24.),
        "2 veces la fuente de la UI"
    );
    assert_eq!(theme.radius.lg, px(0.));
    assert_eq!(theme.text.strong, FontWeight(600.));
    assert_eq!(theme.editor_font_size, px(15.), "el editor no cambia");
}

/// Si lo secundario va más grande que el texto, las filas que lo llevan
/// (el detalle del picker, el atajo del menú) crecen con él y no se pisan.
#[test]
fn las_filas_miden_por_su_texto_mas_grande() {
    let mut config = config::Config::defaults().unwrap();
    config.theme.metrics.text.ui_sm.size = 2.;
    let theme = Theme::with_font(&config, "Mono".into());
    assert_eq!(theme.picker_row_height(), px(30. * 1.4 + 12.));
    assert_eq!(theme.menu_row_height, px(51.), "30 × 1.7");
    assert_eq!(
        theme.panel_row_height,
        px(26.),
        "el árbol no lleva texto secundario"
    );
}

#[test]
fn el_menu_no_sigue_al_padding_del_panel() {
    let mut config = config::Config::defaults().unwrap();
    config.theme.metrics.layout.panel_padding_x = config::Length(20.);
    let theme = Theme::with_font(&config, "Mono".into());
    assert_eq!(theme.layout.menu_item_padding_x, px(8.));
}

#[test]
fn el_margen_del_texto_sale_de_la_config() {
    let mut config = config::Config::defaults().unwrap();
    config.settings.editor.padding = config::Padding {
        horizontal: 20,
        vertical: 0,
    };
    let theme = Theme::with_font(&config, "Mono".into());
    assert_eq!(
        (theme.text_padding_x, theme.text_padding_y),
        (px(20.), px(0.))
    );
}

#[test]
fn el_ancho_del_borde_entre_paneles_sale_de_la_config() {
    let mut config = config::Config::defaults().unwrap();
    let width = |config: &config::Config| Theme::with_font(config, "Mono".into()).pane_border_width;
    assert_eq!(width(&config), px(2.));
    config.settings.panes.border_width = 5;
    assert_eq!(width(&config), px(5.));
}

#[test]
fn sin_ninguna_candidata_no_hay_fuente() {
    assert_eq!(
        first_installed(MONO_FONTS, &installed(&["Cantarell"])),
        None
    );
}

/// El indicador del autoguardado tiene sus propios tokens, con los colores
/// que usaba antes de tenerlos; el claro tiene los mismos nombres de su paleta
/// (snapshots de `metalmind-config`).
#[test]
fn el_indicador_del_autoguardado_sale_del_tema() {
    let mut config = config::Config::defaults().unwrap();
    let theme = Theme::with_font(&config, "Mono".into());
    let ui = &theme.ui;
    assert_eq!(ui.status_saved, ui.status_unfocused_foreground);
    assert_eq!(ui.status_unsaved, ui.modified);
    assert_eq!(ui.status_changed_on_disk, ui.error);
    assert!((theme.layout.status_focus_max_width - 0.5).abs() < f32::EPSILON);
    config.theme.metrics.layout.status_focus_max_width = config::Fraction(0.25);
    let theme = Theme::with_font(&config, "Mono".into());
    assert!((theme.layout.status_focus_max_width - 0.25).abs() < f32::EPSILON);
}
