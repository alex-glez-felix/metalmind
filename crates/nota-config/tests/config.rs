#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs;

use nota_config::{Config, DEFAULT_THEME, FontSize, Paths, Settings, Severity};
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
    fs::write(paths.config_file(), "theme = \"mio\"\n[font]\nsize = 18\n").unwrap();
    fs::write(
        paths.themes_dir().join("mio.toml"),
        "extends = \"nota-claro\"\n",
    )
    .unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.diagnostics, []);
    assert_eq!(
        loaded.value.settings.font.size,
        FontSize::try_from(18.).unwrap()
    );
    assert_eq!(loaded.value.theme.name, "mio");
}

#[test]
fn config_invalida_arranca_con_defaults_y_dice_donde() {
    let (_root, paths) = paths();
    fs::write(paths.config_file(), "theme = nota-claro\n").unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value.settings, Settings::default());
    let [error] = loaded.diagnostics.as_slice() else {
        panic!("{:?}", loaded.diagnostics)
    };
    assert_eq!(error.severity, Severity::Error);
    assert_eq!(error.file, Some(paths.config_file()));
    assert!(
        error
            .to_string()
            .starts_with(&format!("{}:1:", paths.config_file().display()))
    );
}

#[test]
fn un_tema_que_no_carga_cae_al_de_por_defecto() {
    let (_root, paths) = paths();
    fs::write(
        paths.config_file(),
        "theme = \"nada\"\n[editor]\nscrolloff = 4\n",
    )
    .unwrap();
    let loaded = Config::load(&paths).unwrap();
    assert_eq!(loaded.value.settings.editor.scrolloff, 4);
    assert_eq!(loaded.value.theme.name, DEFAULT_THEME);
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].message, "no existe el tema `nada`");
}

#[test]
fn si_el_tema_falla_tambien_se_reportan_sus_avisos() {
    let (_root, paths) = paths();
    fs::write(paths.config_file(), "theme = \"mio\"\n").unwrap();
    fs::write(
        paths.themes_dir().join("mio.toml"),
        "[ui]\nselction = \"#000000\"\n",
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
    assert!(loaded.diagnostics[0].message.starts_with("no se pudo leer"));
}
