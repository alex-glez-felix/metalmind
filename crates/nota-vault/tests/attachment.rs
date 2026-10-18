#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use jiff::civil::{DateTime, date};
use nota_vault::{
    DEFAULT_ATTACHMENT_FOLDER, FileOpError, NewAttachment, SavedAttachment, Source, Vault,
    VaultPath,
};
use pretty_assertions::assert_eq;
use support::{make_tree, settle};
use tempfile::TempDir;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const PDF: &[u8] = b"%PDF-1.7\n";

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn folder() -> VaultPath {
    path(DEFAULT_ATTACHMENT_FOLDER)
}

fn at() -> DateTime {
    date(2026, 10, 18).at(9, 5, 7, 0)
}

fn pasted<'a>(bytes: &'a [u8], note: &'a VaultPath) -> NewAttachment<'a> {
    NewAttachment {
        bytes,
        source: Source::Pasted { note },
        at: at(),
    }
}

fn dropped<'a>(bytes: &'a [u8], file_name: &'a str) -> NewAttachment<'a> {
    NewAttachment {
        bytes,
        source: Source::Dropped { file_name },
        at: at(),
    }
}

fn saved(at: &str, embed: &str) -> SavedAttachment {
    SavedAttachment {
        path: path(at),
        embed: embed.to_owned(),
    }
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

#[test]
fn pegar_crea_la_carpeta_y_devuelve_el_embed() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let note = path("diario/Mi día.md");
    let result = vault
        .save_attachment(&folder(), &pasted(PNG, &note))
        .unwrap();
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
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let result = vault
        .save_attachment(&path("media/img"), &dropped(PNG, "foto.png"))
        .unwrap();
    assert_eq!(result.path, path("media/img/foto-2026-10-18-090507.png"));
}

#[test]
fn nombres_repetidos_no_se_pisan() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let first = vault
        .save_attachment(&folder(), &dropped(PNG, "x.png"))
        .unwrap();
    let second = vault
        .save_attachment(&folder(), &dropped(PDF, "x.pdf"))
        .unwrap();
    let third = vault
        .save_attachment(&folder(), &dropped(PNG, "x.png"))
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
    let result = vault
        .save_attachment(&folder(), &dropped(PNG, "foto.jpg"))
        .unwrap();
    assert_eq!(result.path, path("adjuntos/foto-2026-10-18-090507.png"));
}

#[test]
fn un_tipo_desconocido() {
    let dir = TempDir::new().unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let note = path("nota.md");
    let paste = vault
        .save_attachment(&folder(), &pasted(b"a,b\n1,2\n", &note))
        .unwrap();
    let drop = vault
        .save_attachment(&folder(), &dropped(b"a,b\n1,2\n", "Datos 2026.CSV"))
        .unwrap();
    let nameless = vault
        .save_attachment(&folder(), &dropped(b"", "LEEME"))
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
fn el_embed_lleva_la_carpeta_si_el_nombre_no_basta() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["x-2026-10-18-090507.png"]);
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let result = vault
        .save_attachment(&folder(), &dropped(PNG, "x"))
        .unwrap();
    assert_eq!(
        result,
        saved(
            "adjuntos/x-2026-10-18-090507.png",
            "![[adjuntos/x-2026-10-18-090507.png]]"
        )
    );
}

#[test]
fn la_carpeta_no_puede_ser_un_archivo_ni_un_enlace() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    make_tree(dir.path(), &["adjuntos"]);
    symlink(outside.path(), dir.path().join("enlace")).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    let png = dropped(PNG, "x.png");
    assert!(matches!(
        vault.save_attachment(&folder(), &png),
        Err(FileOpError::NotAFolder(p)) if p == folder()
    ));
    assert!(matches!(
        vault.save_attachment(&path("enlace"), &png),
        Err(FileOpError::Symlink(_))
    ));
    assert!(matches!(
        vault.save_attachment(&path("enlace/dentro"), &png),
        Err(FileOpError::Symlink(_))
    ));
    assert_eq!(names(outside.path()), Vec::<String>::new());
}
