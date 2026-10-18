#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::Path;

use nota_vault::{FileError, create_atomic, read, write_atomic};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn write_text(path: &Path, text: &str) -> Result<(), FileError> {
    write_atomic(path, |out| out.write_all(text.as_bytes()))
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

/// Lo que hay en el directorio, ordenado; un temporal olvidado aparece aquí.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn crea_un_archivo_nuevo() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nueva.md");
    write_text(&path, "# Hola\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Hola\n");
    assert_eq!(entries(dir.path()), ["nueva.md"]);
}

#[test]
fn reemplaza_el_contenido() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, "un texto viejo y bastante más largo\n").unwrap();
    write_text(&path, "nuevo\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "nuevo\n");
    assert_eq!(entries(dir.path()), ["nota.md"]);
}

#[test]
fn conserva_los_permisos_del_original() {
    let dir = TempDir::new().unwrap();
    for original in [0o600, 0o640, 0o755] {
        let path = dir.path().join(format!("{original:o}.md"));
        fs::write(&path, "x").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(original)).unwrap();
        write_text(&path, "y").unwrap();
        assert_eq!(mode(&path), original, "{original:o}");
    }
}

#[test]
fn un_archivo_nuevo_tiene_los_permisos_de_cualquier_archivo_nuevo() {
    let dir = TempDir::new().unwrap();
    let reference = dir.path().join("referencia");
    fs::File::create(&reference).unwrap();
    let path = dir.path().join("nueva.md");
    write_text(&path, "x").unwrap();
    assert_eq!(mode(&path), mode(&reference));
}

#[test]
fn si_falla_la_escritura_el_original_queda_intacto() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, "original\n").unwrap();
    let error = write_atomic(&path, |out| {
        out.write_all(&vec![b'z'; 1 << 20])?;
        Err(io::Error::other("disco lleno"))
    })
    .unwrap_err();
    assert!(
        matches!(&error, FileError::Write { source, .. } if source.to_string() == "disco lleno"),
        "{error:?}"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "original\n");
    assert_eq!(entries(dir.path()), ["nota.md"]);
}

#[test]
fn si_falla_el_rename_no_quedan_temporales() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("carpeta");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("dentro.md"), "intacto").unwrap();
    assert!(matches!(
        write_text(&path, "x"),
        Err(FileError::Write { .. })
    ));
    assert_eq!(entries(dir.path()), ["carpeta"]);
    assert_eq!(
        fs::read_to_string(path.join("dentro.md")).unwrap(),
        "intacto"
    );
}

#[test]
fn falla_si_el_directorio_no_existe() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("no-existe").join("nota.md");
    assert!(matches!(
        write_text(&path, "x"),
        Err(FileError::Write { .. })
    ));
}

#[test]
fn a_traves_de_un_enlace_escribe_en_el_destino() {
    let dir = TempDir::new().unwrap();
    let real = dir.path().join("real.md");
    let link = dir.path().join("enlace.md");
    fs::write(&real, "viejo").unwrap();
    symlink(&real, &link).unwrap();
    write_text(&link, "nuevo").unwrap();
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&real).unwrap(), "nuevo");
    assert_eq!(entries(dir.path()), ["enlace.md", "real.md"]);
}

#[test]
fn ruta_relativa_sin_directorio() {
    let dir = TempDir::new().unwrap();
    let previous = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let result = write_text(Path::new("suelta.md"), "x");
    std::env::set_current_dir(previous).unwrap();
    result.unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("suelta.md")).unwrap(),
        "x"
    );
}

#[test]
fn leer_devuelve_los_bytes_tal_cual() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    let text = "\u{feff}línea con CRLF\r\nsin salto final";
    fs::write(&path, text).unwrap();
    assert_eq!(read(&path).unwrap().as_deref(), Some(text));
    write_text(&path, text).unwrap();
    assert_eq!(fs::read(&path).unwrap(), text.as_bytes());
}

#[test]
fn leer_un_archivo_que_no_existe_es_none() {
    let dir = TempDir::new().unwrap();
    assert_eq!(read(&dir.path().join("nueva.md")).unwrap(), None);
}

#[test]
fn leer_algo_que_no_es_utf8_falla() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("binario.md");
    fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
    assert!(matches!(read(&path), Err(FileError::NotUtf8 { .. })));
}

#[test]
fn leer_una_carpeta_falla() {
    let dir = TempDir::new().unwrap();
    assert!(matches!(read(dir.path()), Err(FileError::Read { .. })));
}

fn create_text(path: &Path, text: &str) -> Result<(), FileError> {
    create_atomic(path, |out| out.write_all(text.as_bytes()))
}

#[test]
fn crear_escribe_un_archivo_nuevo() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nueva.md");
    create_text(&path, "# Hola\n").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Hola\n");
    let new_file = dir.path().join("comparar");
    fs::write(&new_file, "").unwrap();
    assert_eq!(
        mode(&path),
        mode(&new_file),
        "los permisos de un archivo nuevo"
    );
}

#[test]
fn crear_no_pisa_lo_que_ya_existe() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ocupado.md");
    fs::write(&path, "de otro\n").unwrap();
    let error = create_text(&path, "nuevo\n").unwrap_err();
    assert!(matches!(error, FileError::Exists { .. }), "{error:?}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "de otro\n");
    assert_eq!(entries(dir.path()), ["ocupado.md"]);
    symlink(dir.path().join("no-existe"), dir.path().join("enlace.md")).unwrap();
    let error = create_text(&dir.path().join("enlace.md"), "x").unwrap_err();
    assert!(matches!(error, FileError::Exists { .. }), "{error:?}");
    assert!(!dir.path().join("no-existe").exists());
}

/// Otro proceso crea el archivo mientras se escribe: el rename final no lo
/// pisa, aunque al empezar no existía.
#[test]
fn crear_no_pisa_lo_que_aparece_mientras_se_escribe() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("carrera.md");
    let error = create_atomic(&path, |out| {
        fs::write(&path, "llegó primero\n")?;
        out.write_all(b"tarde\n")
    })
    .unwrap_err();
    assert!(matches!(error, FileError::Exists { .. }), "{error:?}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "llegó primero\n");
    assert_eq!(entries(dir.path()), ["carrera.md"]);
}
