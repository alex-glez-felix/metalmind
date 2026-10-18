#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use jiff::civil::{DateTime, date};
use metalmind_vault::{
    AttachmentError, FileOpError, NewAttachment, SavedAttachment, Vault, VaultOptions, VaultPath,
    VaultUpdate,
};
use pretty_assertions::assert_eq;
use support::{make_tree, settle};
use tempfile::TempDir;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const PDF: &[u8] = b"%PDF-1.7\n";

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn at() -> DateTime {
    date(2026, 10, 18).at(9, 5, 7, 0)
}

fn pasted<'a>(bytes: &'a [u8], note: &'a VaultPath) -> NewAttachment<'a> {
    NewAttachment {
        bytes,
        note,
        file_name: None,
        at: at(),
    }
}

fn dropped<'a>(bytes: &'a [u8], file_name: &'a str, note: &'a VaultPath) -> NewAttachment<'a> {
    NewAttachment {
        file_name: Some(file_name),
        ..pasted(bytes, note)
    }
}

fn saved(at: &str, embed: &str) -> SavedAttachment {
    SavedAttachment {
        path: path(at),
        embed: embed.to_owned(),
    }
}

fn open_with_folder(root: &Path, folder: &str) -> (Vault, Receiver<VaultUpdate>) {
    let options = VaultOptions {
        attachments_folder: path(folder),
        ..VaultOptions::default()
    };
    Vault::open_with(root, options).unwrap()
}

/// Los nombres de `dir`, ocultos incluidos: un temporal olvidado se vería.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Espera a que el watcher deje los huérfanos en `expected`.
fn orphans_settle(vault: &Vault, updates: &Receiver<VaultUpdate>, expected: &[&str]) {
    let expected: Vec<VaultPath> = expected.iter().map(|p| path(p)).collect();
    let deadline = Instant::now() + Duration::from_secs(10);
    while vault.orphan_attachments() != expected {
        let left = deadline.saturating_duration_since(Instant::now());
        if updates.recv_timeout(left).is_err() {
            assert_eq!(vault.orphan_attachments(), expected, "el watcher no llegó");
        }
    }
}

#[test]
fn pegar_crea_la_carpeta_y_devuelve_el_embed() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let note = path("diario/Mi día.md");
    let result = vault.save_attachment(&pasted(PNG, &note)).unwrap();
    assert_eq!(
        result,
        saved(
            "adjuntos/Mi-día-2026-10-18-090507.png",
            "![[Mi-día-2026-10-18-090507.png]]"
        )
    );
    assert_eq!(
        fs::read(dir.path().join(result.path.as_str())).unwrap(),
        PNG
    );
    assert_eq!(
        names(&dir.path().join("adjuntos")),
        ["Mi-día-2026-10-18-090507.png"]
    );
    settle(
        &vault,
        &updates,
        &[
            "adjuntos/",
            "adjuntos/Mi-día-2026-10-18-090507.png (adjunto)",
        ],
    );
}

#[test]
fn una_carpeta_anidada_que_no_existe() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = open_with_folder(dir.path(), "media/img");
    let note = path("nota.md");
    let result = vault
        .save_attachment(&dropped(PNG, "foto.png", &note))
        .unwrap();
    assert_eq!(result.path, path("media/img/foto-2026-10-18-090507.png"));
}

#[test]
fn nombres_repetidos_no_se_pisan() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let first = vault
        .save_attachment(&dropped(PNG, "x.png", &note))
        .unwrap();
    let second = vault
        .save_attachment(&dropped(PDF, "x.pdf", &note))
        .unwrap();
    let third = vault
        .save_attachment(&dropped(PNG, "x.png", &note))
        .unwrap();
    assert_eq!(
        [first.path, second.path, third.path.clone()],
        [
            path("adjuntos/x-2026-10-18-090507.png"),
            path("adjuntos/x-2026-10-18-090507.pdf"),
            path("adjuntos/x-2026-10-18-090507-1.png"),
        ]
    );
    assert_eq!(
        fs::read(dir.path().join("adjuntos/x-2026-10-18-090507.png")).unwrap(),
        PNG
    );
    assert_eq!(fs::read(dir.path().join(third.path.as_str())).unwrap(), PNG);
    assert_eq!(names(&dir.path().join("adjuntos")).len(), 3);
}

#[test]
fn la_extension_sale_del_contenido() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let result = vault
        .save_attachment(&dropped(PNG, "foto.jpg", &note))
        .unwrap();
    assert_eq!(result.path, path("adjuntos/foto-2026-10-18-090507.png"));
}

#[test]
fn una_firma_compartida_deja_la_extension_propia() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let raw = b"II*\0\x10\0\0\0CR\x02\0";
    let cr2 = vault
        .save_attachment(&dropped(raw, "IMG_0001.CR2", &note))
        .unwrap();
    let tiff = vault.save_attachment(&pasted(raw, &note)).unwrap();
    let three_gp = vault
        .save_attachment(&dropped(
            b"\0\0\0\x18ftyp3gp4\0\0\x02\0isom3gp4",
            "video.3gp",
            &note,
        ))
        .unwrap();
    let html = vault
        .save_attachment(&dropped(
            b"<!DOCTYPE html><link href=\"data:image/svg+xml,<svg/>\">",
            "page.html",
            &note,
        ))
        .unwrap();
    assert_eq!(
        [cr2.path, tiff.path, three_gp.path, html.path],
        [
            path("adjuntos/IMG_0001-2026-10-18-090507.cr2"),
            path("adjuntos/nota-2026-10-18-090507.tiff"),
            path("adjuntos/video-2026-10-18-090507.3gp"),
            path("adjuntos/page-2026-10-18-090507.html"),
        ]
    );
}

#[test]
fn un_tipo_desconocido() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let paste = vault
        .save_attachment(&pasted(b"a,b\n1,2\n", &note))
        .unwrap();
    let drop = vault
        .save_attachment(&dropped(b"a,b\n1,2\n", "Datos 2026.CSV", &note))
        .unwrap();
    let nameless = vault
        .save_attachment(&dropped(b"", "LEEME", &note))
        .unwrap();
    assert_eq!(
        [paste.path, drop.path, nameless.path],
        [
            path("adjuntos/nota-2026-10-18-090507.bin"),
            path("adjuntos/Datos-2026-2026-10-18-090507.csv"),
            path("adjuntos/LEEME-2026-10-18-090507.bin"),
        ]
    );
}

#[test]
fn un_md_arrastrado_no_es_un_adjunto() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let result = vault.save_attachment(&dropped(b"\xff\xfe no es UTF-8", "Otra.MD", &note));
    assert!(
        matches!(&result, Err(AttachmentError::Note(name)) if name == "Otra-2026-10-18-090507.md"),
        "{result:?}"
    );
    assert!(!dir.path().join("adjuntos").exists());
}

#[test]
fn un_nombre_largo_en_cualquier_alfabeto_cabe() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let long = format!("{}.png", "𝒜".repeat(100));
    let result = vault.save_attachment(&dropped(PNG, &long, &note)).unwrap();
    assert_eq!(
        fs::read(dir.path().join(result.path.as_str())).unwrap(),
        PNG
    );
}

#[test]
fn el_embed_lleva_la_carpeta_si_el_nombre_no_basta() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["x-2026-10-18-090507.png"]);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let result = vault.save_attachment(&dropped(PNG, "x", &note)).unwrap();
    assert_eq!(
        result,
        saved(
            "adjuntos/x-2026-10-18-090507.png",
            "![[adjuntos/x-2026-10-18-090507.png]]"
        )
    );
}

#[test]
fn el_embed_se_resuelve_desde_la_nota() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["otra/x-2026-10-18-090507.png"]);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let inside = path("adjuntos/nota.md");
    let result = vault.save_attachment(&dropped(PNG, "x", &inside)).unwrap();
    assert_eq!(result.embed, "![[x-2026-10-18-090507.png]]");
}

#[test]
fn la_carpeta_no_puede_ser_un_archivo_ni_un_enlace() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    make_tree(dir.path(), &["adjuntos"]);
    symlink(outside.path(), dir.path().join("enlace")).unwrap();
    let note = path("nota.md");
    let png = dropped(PNG, "x.png", &note);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert!(matches!(
        vault.save_attachment(&png),
        Err(AttachmentError::Folder(FileOpError::NotAFolder(p))) if p == path("adjuntos")
    ));
    for folder in ["enlace", "enlace/dentro"] {
        let (vault, _updates) = open_with_folder(dir.path(), folder);
        assert!(matches!(
            vault.save_attachment(&png),
            Err(AttachmentError::Folder(FileOpError::Symlink(_)))
        ));
    }
    assert_eq!(names(outside.path()), Vec::<String>::new());
}

#[test]
fn lo_que_el_vault_ignora_no_se_guarda() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitignore"), "privado/\n*.pdf\n").unwrap();
    let note = path("nota.md");
    let (vault, _updates) = open_with_folder(dir.path(), "privado/img");
    let result = vault.save_attachment(&dropped(PNG, "x.png", &note));
    assert!(
        matches!(&result, Err(AttachmentError::Ignored(p)) if p.as_str() == "privado/img/x-2026-10-18-090507.png"),
        "{result:?}"
    );
    assert!(
        !dir.path().join("privado").exists(),
        "quita las carpetas que creó"
    );
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let result = vault.save_attachment(&dropped(PDF, "informe.pdf", &note));
    assert!(
        matches!(result, Err(AttachmentError::Ignored(_))),
        "{result:?}"
    );
    assert!(!dir.path().join("adjuntos").exists());
}

#[test]
fn una_carpeta_que_un_wikilink_no_nombra() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a#b/", "otra/x-2026-10-18-090507.png"]);
    let note = path("nota.md");
    let (vault, _updates) = open_with_folder(dir.path(), "a#b");
    let result = vault.save_attachment(&dropped(PNG, "x.png", &note));
    assert!(
        matches!(result, Err(AttachmentError::Unnameable(_))),
        "{result:?}"
    );
    assert_eq!(names(&dir.path().join("a#b")), Vec::<String>::new());
}

#[test]
fn huerfanos_con_el_watcher() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["adjuntos/viejo.png", "fuera.png"]);
    fs::write(dir.path().join("a.md"), "![[viejo.png]]\n").unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(vault.orphan_attachments(), Vec::<VaultPath>::new());

    let note = path("a.md");
    let new = vault.save_attachment(&pasted(PNG, &note)).unwrap();
    orphans_settle(&vault, &updates, &[new.path.as_str()]);

    fs::write(
        dir.path().join("a.md"),
        format!("![[viejo.png]] {}\n", new.embed),
    )
    .unwrap();
    orphans_settle(&vault, &updates, &[]);

    fs::write(dir.path().join("a.md"), "nada\n").unwrap();
    orphans_settle(
        &vault,
        &updates,
        &["adjuntos/a-2026-10-18-090507.png", "adjuntos/viejo.png"],
    );
}
