use std::fs;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;

#[test]
fn abrir_un_archivo_que_no_existe_da_un_buffer_vacio() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nueva.md");
    let document = Document::open(&path).unwrap();
    assert_eq!(document.buffer().to_string(), "");
    assert_eq!(document.path(), path);
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
    assert_eq!(document.buffer().to_string(), "# Título\n\ntexto\n");
    assert!(document.exists_on_disk());
    assert_eq!(
        document.stats(),
        FileStats {
            lines: 4,
            bytes: 17
        }
    );
    assert_eq!(document.stats().to_string(), "4L, 17B");
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
