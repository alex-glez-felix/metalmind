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
