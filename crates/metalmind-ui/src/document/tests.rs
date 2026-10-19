use std::fs;
use std::path::Path;

use pretty_assertions::assert_eq;
use rstest::rstest;
use tempfile::TempDir;

use super::*;

#[test]
fn abrir_un_archivo_que_no_existe_da_un_buffer_vacio() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nueva.md");
    let document = Document::open(&path).unwrap();
    assert_eq!(document.buffer().to_string(), "");
    assert_eq!(document.path(), Some(path.as_path()));
    assert!(!document.exists_on_disk());
    assert!(!document.is_modified());
    assert!(!path.exists());
}

#[test]
fn abrir_lee_el_archivo() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, "# Título\n\ntexto\n").unwrap();
    let document = Document::open(&path).unwrap();
    assert_eq!(document.buffer().to_string(), "# Título\n\ntexto");
    assert!(document.exists_on_disk());
    assert_eq!(
        document.stats(),
        FileStats {
            lines: 3,
            bytes: 17
        }
    );
    assert_eq!(document.stats().to_string(), "3L, 17B");
}

/// Como el `fixeol` de vim: el `\n` final no es una línea más y guardar lo
/// pone si falta.
#[rstest]
#[case::con_salto("uno\ndos\n", "uno\ndos", "uno\ndos\n")]
#[case::sin_salto("uno\ndos", "uno\ndos", "uno\ndos\n")]
#[case::linea_vacia_al_final("uno\n\n", "uno\n", "uno\n\n")]
#[case::vacio("", "", "")]
#[case::solo_una_linea_vacia("\n", "", "\n")]
fn el_salto_final_no_entra_al_buffer(
    #[case] on_disk: &str,
    #[case] buffer: &str,
    #[case] saved: &str,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, on_disk).unwrap();
    let mut document = Document::open(&path).unwrap();
    assert_eq!(document.buffer().to_string(), buffer);
    let stats = document.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), saved);
    assert_eq!(stats.bytes, saved.len());
    assert_eq!(stats.lines, saved.lines().count());
}

#[cfg(unix)]
#[test]
fn un_enlace_es_el_archivo_al_que_apunta_ahora() {
    let dir = TempDir::new().unwrap();
    let (a, b, link) = (
        dir.path().join("a.md"),
        dir.path().join("b.md"),
        dir.path().join("enlace.md"),
    );
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    std::os::unix::fs::symlink(&a, &link).unwrap();
    let document = Document::open(&link).unwrap();
    assert!(document.is_file(&a));
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&b, &link).unwrap();
    assert!(document.is_file(&b), "guardar ya escribiría en b.md");
    assert!(!document.is_file(&a));
}

#[test]
fn una_linea_vacia_cuenta_como_una_linea() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, "\n").unwrap();
    let document = Document::open(&path).unwrap();
    assert_eq!(document.stats(), FileStats { lines: 1, bytes: 1 });
}

#[rstest]
#[case::con_salto("uno\n", false, FileStats { lines: 1, bytes: 4 })]
#[case::sin_salto("uno", true, FileStats { lines: 1, bytes: 3 })]
#[case::vacio("", false, FileStats { lines: 0, bytes: 0 })]
fn al_abrir_avisa_lo_que_hay_en_disco(
    #[case] on_disk: &str,
    #[case] noeol: bool,
    #[case] stats: FileStats,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, on_disk).unwrap();
    let mut document = Document::open(&path).unwrap();
    assert_eq!(document.noeol(), noeol);
    assert_eq!(document.opened_stats(), stats);
    document.save().unwrap();
    assert!(!document.noeol(), "guardar pone el \\n que faltaba");
}

#[test]
fn un_buffer_sin_nombre_no_se_guarda() {
    let mut document = Document::unnamed();
    assert_eq!(document.path(), None);
    assert_eq!(document.display_name(), "[No Name]");
    assert!(!document.is_file(Path::new("a.md")));
    assert!(matches!(document.save(), Err(SaveError::NoFileName)));
    assert_eq!(SaveError::NoFileName.to_string(), "E32: No file name");
}

#[test]
fn w_con_ruta_le_da_archivo_a_un_buffer_sin_nombre() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nueva.md");
    let mut document = Document::with_path(None, "hola");
    document.save_to(&path).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hola\n");
    assert_eq!(document.path(), Some(path.as_path()));
    assert!(!document.is_modified());
}

#[test]
fn w_con_otra_ruta_escribe_una_copia_sin_pisar_nada() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.md");
    fs::write(&path, "a\n").unwrap();
    fs::write(dir.path().join("ocupado.md"), "no\n").unwrap();
    let mut document = Document::open(&path).unwrap();
    let copy = dir.path().join("copia.md");
    document.save_to(&copy).unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), "a\n");
    assert_eq!(document.path(), Some(path.as_path()), "sigue en el suyo");
    let taken = document.save_to(&dir.path().join("ocupado.md"));
    assert!(matches!(taken, Err(SaveError::Exists)));
    assert_eq!(
        SaveError::Exists.to_string(),
        "E13: File exists (add ! to override)"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("ocupado.md")).unwrap(),
        "no\n"
    );
    assert!(Document::unnamed().save_to(&path).is_err());
}
