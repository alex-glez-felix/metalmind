#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs;

use nota_config::{Config, DEFAULT_THEME, FontSize, Location, Paths, Settings, Severity};
use pretty_assertions::assert_eq;

fn paths() -> (tempfile::TempDir, Paths) {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::under(root.path());
    fs::create_dir_all(paths.themes_dir()).unwrap();
    (root, paths)
}

#[test]
fn sin_archivo_arranca_con_los_defaults() {
    let root = tempfile::tempdir().unwrap();
    let loaded = Config::load(&Paths::under(root.path())).unwrap();
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value, Config::defaults().unwrap());
    assert_eq!(loaded.value.settings, Settings::default());
    assert_eq!(loaded.value.theme.name, DEFAULT_THEME);
}

#[test]
fn lee_la_config_y_el_tema_del_usuario() {
    let (_root, paths) = paths();
    fs::write(paths.config_file(), "theme: mio\nfont:\n  size: 18\n").unwrap();
    fs::write(paths.themes_dir().join("mio.yaml"), "extends: nota-claro\n").unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(
        loaded.value.settings.font.size,
        FontSize::try_from(18.).unwrap()
    );
    assert_eq!(loaded.value.theme.name, "mio");
}

#[test]
fn un_config_en_json_puro_carga_igual_que_en_yaml() {
    let (_root, paths) = paths();
    fs::write(
        paths.config_file(),
        "theme: nota-claro\neditor:\n  tabstop: 2\n",
    )
    .unwrap();
    let yaml = Config::load(&paths).unwrap();
    fs::write(
        paths.config_file(),
        r#"{ "theme": "nota-claro", "editor": { "tabstop": 2 } }"#,
    )
    .unwrap();
    let json = Config::load(&paths).unwrap();
    assert_eq!(json, yaml);
    assert_eq!(json.diagnostics, []);
    assert_eq!(json.value.theme.name, "nota-claro");
}

#[test]
fn config_yml_tambien_vale_y_con_los_dos_gana_yaml() {
    let (_root, paths) = paths();
    let yml = paths.config.join("config.yml");
    fs::write(&yml, "theme: nota-claro\n").unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(loaded.value.theme.name, "nota-claro");

    fs::write(paths.config_file(), "editor:\n  scrolloff: 2\n").unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value.theme.name, DEFAULT_THEME);
    assert_eq!(loaded.value.settings.editor.scrolloff, 2);
    let [warning] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.file, Some(yml));
}

#[test]
fn un_config_toml_ya_no_se_lee_y_se_avisa() {
    let (_root, paths) = paths();
    let toml = paths.config.join("config.toml");
    fs::write(&toml, "theme = \"nota-claro\"\n").unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value, Config::defaults().unwrap());
    let [warning] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.file, Some(toml));
    assert!(warning.message.contains("`config.yaml`"));
}

#[test]
fn config_rota_arranca_con_defaults_y_dice_donde() {
    let (_root, paths) = paths();
    fs::write(paths.config_file(), "theme: nota-claro\n  font: 3\n").unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value, Config::defaults().unwrap());
    let [error] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(error.severity, Severity::Error);
    assert_eq!(error.file, Some(paths.config_file()));
    assert_eq!(error.location, Some(Location { line: 2, column: 7 }));
    assert!(
        error
            .to_string()
            .starts_with(&format!("{}:2:7: error: ", paths.config_file().display()))
    );
}

#[test]
fn un_tema_que_no_carga_cae_al_de_por_defecto() {
    let (_root, paths) = paths();
    fs::write(
        paths.config_file(),
        "theme: nada\neditor:\n  scrolloff: 4\n",
    )
    .unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value.settings.editor.scrolloff, 4);
    assert_eq!(loaded.value.theme.name, DEFAULT_THEME);
    let [error] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(error.message, "no existe el tema `nada`");
    assert_eq!(error.file, Some(paths.config_file()));
    assert_eq!(error.location, Some(Location { line: 1, column: 8 }));
}

#[test]
fn theme_no_es_el_nombre_de_un_tema_y_no_un_booleano() {
    let (_root, paths) = paths();
    fs::write(paths.config_file(), "theme: NO\n").unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value.settings.theme(), "NO");
    let [error] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(error.message, "no existe el tema `NO`");
}

#[test]
fn si_el_tema_falla_tambien_se_reportan_sus_avisos() {
    let (_root, paths) = paths();
    fs::write(paths.config_file(), "theme: mio\n").unwrap();
    fs::write(
        paths.themes_dir().join("mio.yaml"),
        "ui:\n  selction: \"#000000\"\n",
    )
    .unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value.theme.name, DEFAULT_THEME);
    let severities: Vec<_> = loaded.diagnostics.iter().map(|d| d.severity).collect();
    assert_eq!(severities, [Severity::Warning, Severity::Error]);
    assert!(loaded.diagnostics[0].message.contains("ui.selction"));
}

#[test]
fn un_config_ilegible_se_reporta() {
    let (_root, paths) = paths();
    fs::create_dir(paths.config_file()).unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value.settings, Settings::default());
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].file, Some(paths.config_file()));
    assert!(loaded.diagnostics[0].message.starts_with("no se pudo leer"));
}
