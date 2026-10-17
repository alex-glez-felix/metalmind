use std::fs;

use nota_vim::Command;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;

fn document(text: &str) -> Document {
    Document::new(Path::new("prueba.md"), text)
}

fn keys(document: &mut Document, text: &str) -> Option<KeyOutcome> {
    let mut last = None;
    for c in text.chars() {
        let key = match c {
            '⎋' => Key::Escape,
            '⏎' => Key::Enter,
            '⌫' => Key::Backspace,
            _ => Key::Char(c),
        };
        last = Some(document.feed(key).unwrap());
    }
    last
}

#[test]
fn el_texto_compuesto_entra_por_el_motor() {
    let mut document = document("hola\n");
    keys(&mut document, "A");
    assert_eq!(
        document.type_text("ñá").unwrap(),
        Some(KeyOutcome::Inserted)
    );
    keys(&mut document, "⎋");
    assert_eq!(document.buffer().to_string(), "holañá\n");
    assert_eq!(document.mode(), Mode::Normal);
}

#[test]
fn en_normal_el_texto_son_comandos() {
    let mut document = document("uno dos\n");
    assert!(matches!(
        document.type_text("w").unwrap(),
        Some(KeyOutcome::Executed(_))
    ));
    assert_eq!(document.cursor(), ByteOffset::new(4));
}

#[test]
fn pegar_en_normal_es_un_paso_de_undo() {
    let mut document = document("ab\n");
    document.paste("XY").unwrap();
    assert_eq!(document.buffer().to_string(), "XYab\n");
    assert_eq!(document.cursor(), ByteOffset::new(1));
    keys(&mut document, "u");
    assert_eq!(document.buffer().to_string(), "ab\n");
}

#[test]
fn pegar_en_insert_se_deshace_con_lo_escrito() {
    let mut document = document("ab\n");
    keys(&mut document, "iz");
    document.paste("ñ").unwrap();
    keys(&mut document, "q⎋");
    assert_eq!(document.buffer().to_string(), "zñqab\n");
    keys(&mut document, "u");
    assert_eq!(document.buffer().to_string(), "ab\n");
}

#[test]
fn pegar_en_la_linea_de_comandos_no_toca_el_texto() {
    let mut document = document("ab\n");
    keys(&mut document, ":");
    document.paste("wq").unwrap();
    assert_eq!(document.command_line(), Some("wq"));
    assert_eq!(document.buffer().to_string(), "ab\n");
    assert!(!document.is_modified());
}

#[test]
fn seleccion_con_raton_y_copia() {
    let mut document = document("hola mundo\nadiós\n");
    assert_eq!(document.selected_text().unwrap(), None);
    document.select(ByteOffset::new(5), ByteOffset::new(10));
    assert_eq!(document.selected_text().unwrap().as_deref(), Some("mundo"));
    document.select(ByteOffset::new(11), ByteOffset::new(11));
    assert_eq!(document.current_line().unwrap(), "adiós\n");
}

#[test]
fn en_normal_el_cursor_no_queda_sobre_el_salto() {
    let mut document = document("abc\n\nxyz\n");
    document.select(ByteOffset::new(3), ByteOffset::new(3));
    assert_eq!(document.cursor(), ByteOffset::new(2));
    document.select(ByteOffset::new(4), ByteOffset::new(4));
    assert_eq!(document.cursor(), ByteOffset::new(4));
    keys(&mut document, "A");
    document.select(ByteOffset::new(3), ByteOffset::new(3));
    assert_eq!(document.cursor(), ByteOffset::new(3));
}

#[test]
fn dos_puntos_w_pide_guardar() {
    let mut document = document("ab\n");
    assert_eq!(
        keys(&mut document, ":w⏎"),
        Some(KeyOutcome::Command(Command::Save))
    );
    assert_eq!(document.mode(), Mode::Normal);
    assert_eq!(
        keys(&mut document, ":wq⏎"),
        Some(KeyOutcome::Command(Command::SaveAndQuit))
    );
    assert_eq!(
        keys(&mut document, ":qx⌫!⏎"),
        Some(KeyOutcome::Command(Command::ForceQuit))
    );
}

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

#[test]
fn editar_marca_modificado_y_guardar_lo_limpia() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nota.md");
    fs::write(&path, "uno\n").unwrap();
    let mut document = Document::open(&path).unwrap();
    keys(&mut document, "Adós⎋");
    assert!(document.is_modified());
    assert_eq!(fs::read_to_string(&path).unwrap(), "uno\n");
    let stats = document.save().unwrap();
    assert!(!document.is_modified());
    assert_eq!(stats, FileStats { lines: 2, bytes: 8 });
    assert_eq!(fs::read_to_string(&path).unwrap(), "unodós\n");
}

#[test]
fn guardar_crea_el_archivo_nuevo() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nueva.md");
    let mut document = Document::open(&path).unwrap();
    keys(&mut document, "ihola⎋");
    document.save().unwrap();
    assert!(document.exists_on_disk());
    assert_eq!(fs::read_to_string(&path).unwrap(), "hola");
}

#[test]
fn deshacer_tras_guardar_vuelve_a_marcar_modificado() {
    let dir = TempDir::new().unwrap();
    let mut document = Document::open(&dir.path().join("nota.md")).unwrap();
    keys(&mut document, "ix⎋");
    document.save().unwrap();
    keys(&mut document, "u");
    assert!(document.is_modified());
}

#[test]
fn si_guardar_falla_sigue_modificado() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("no-existe").join("nota.md");
    let mut document = Document::open(&path).unwrap();
    keys(&mut document, "ix⎋");
    assert!(document.save().is_err());
    assert!(document.is_modified());
    assert!(!document.exists_on_disk());
}
