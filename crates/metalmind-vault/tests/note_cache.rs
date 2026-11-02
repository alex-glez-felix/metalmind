//! La caché de las notas vista desde `Vault::open_with`: da los mismos índices
//! que leer todo, no relee lo que no cambió y nunca le gana a los `.md`.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use metalmind_vault::tags::TagKey;
use metalmind_vault::{LinkIndex, LinkTarget, Vault, VaultOptions, VaultPath};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn path(text: &str) -> VaultPath {
    VaultPath::new(text).unwrap()
}

/// Un vault con notas de mtime viejo: solo así su sello es de fiar y la caché
/// evita leerlas.
struct Fixture {
    vault: TempDir,
    data: TempDir,
}

impl Fixture {
    fn new(notes: &[(&str, &str)]) -> Self {
        let fixture = Self {
            vault: TempDir::new().unwrap(),
            data: TempDir::new().unwrap(),
        };
        for (name, text) in notes {
            fixture.write(name, text);
        }
        fixture
    }

    fn at(&self, name: &str) -> PathBuf {
        self.vault.path().join(name)
    }

    fn cache_file(&self) -> PathBuf {
        self.data.path().join("cache/notes")
    }

    /// Escribe `name` con un mtime viejo pero distinto al de antes.
    fn write(&self, name: &str, text: &str) {
        let full = self.at(name);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(&full, text).unwrap();
        set_mtime(&full, Duration::from_secs(3_600));
    }

    fn options(&self) -> VaultOptions {
        VaultOptions {
            cache: Some(self.cache_file()),
            ..VaultOptions::default()
        }
    }

    fn open(&self) -> Vault {
        Vault::open_with(self.vault.path(), self.options())
            .unwrap()
            .0
    }

    /// Abre y cierra: al soltar el vault, la caché ya quedó escrita.
    fn open_and_close(&self) {
        drop(self.open());
    }
}

fn set_mtime(file: &Path, ago: Duration) {
    let handle = File::options().write(true).open(file).unwrap();
    handle.set_modified(SystemTime::now() - ago).unwrap();
}

fn targets(links: &LinkIndex, note: &str) -> Vec<String> {
    links
        .outgoing(&path(note))
        .iter()
        .map(|link| link.target_text.clone())
        .collect()
}

const NOTES: &[(&str, &str)] = &[
    (
        "a.md",
        "---\ntags: [uno]\nfecha: 2026\n---\n[[b]] [[falta]] [[c#Sec]] ![[img.png]] #dos",
    ),
    ("b.md", "[md](sub/c.md) #dos/tres"),
    ("sub/c.md", "# Sec\n[[a]] [[d]]"),
    ("otra/c.md", "empata con sub/c.md"),
    ("d.md", ""),
    (
        "tags/dos.md",
        "---\naliases: [deux]\nparents: [raiz]\ncolor: red\n---\nTexto de la página [[a]]",
    ),
    ("tags/no vale.md", "#cuatro"),
    ("img.png", "no es nota"),
];

#[test]
fn abrir_con_cache_da_los_mismos_indices_que_sin_ella() {
    let fixture = Fixture::new(NOTES);
    let (plain, _updates) = Vault::open(fixture.vault.path()).unwrap();

    let building = fixture.open();
    assert_eq!(*building.links(), *plain.links());
    assert_eq!(*building.tags(), *plain.tags());
    drop(building);
    assert!(fixture.cache_file().exists());

    let cached = fixture.open();
    assert_eq!(*cached.links(), *plain.links());
    assert_eq!(*cached.tags(), *plain.tags());
    assert!(matches!(
        cached.links().outgoing(&path("a.md"))[0].target,
        LinkTarget::Resolved(_)
    ));
}

/// Mismo tamaño y mismo mtime: lo único que explica ver el texto de antes es
/// que la nota no se leyó.
#[test]
fn una_nota_igual_por_fuera_no_se_relee() {
    let fixture = Fixture::new(&[("a.md", "[[uno]] #x"), ("b.md", "y")]);
    fixture.open_and_close();

    let stamp = fs::metadata(fixture.at("a.md"))
        .unwrap()
        .modified()
        .unwrap();
    fs::write(fixture.at("a.md"), "[[dos]] #z").unwrap();
    File::options()
        .write(true)
        .open(fixture.at("a.md"))
        .unwrap()
        .set_modified(stamp)
        .unwrap();

    let vault = fixture.open();
    assert_eq!(targets(&vault.links(), "a.md"), ["uno"]);
    assert!(vault.tags().index().tag(&TagKey::new("x")).is_some());
}

#[test]
fn lo_que_cambio_con_el_vault_cerrado_se_ve() {
    let fixture = Fixture::new(&[("a.md", "[[uno]]"), ("b.md", "#x"), ("c.md", "[[a]]")]);
    fixture.open_and_close();

    fixture.write("a.md", "[[dos]] [[tres]]");
    fs::remove_file(fixture.at("b.md")).unwrap();
    fixture.write("nueva.md", "[[c]] #y");

    let vault = fixture.open();
    assert_eq!(targets(&vault.links(), "a.md"), ["dos", "tres"]);
    assert_eq!(targets(&vault.links(), "b.md"), Vec::<String>::new());
    assert_eq!(targets(&vault.links(), "nueva.md"), ["c"]);
    let tags = vault.tags();
    assert!(tags.index().tag(&TagKey::new("x")).is_none());
    assert!(tags.index().tag(&TagKey::new("y")).is_some());
    drop(vault);

    let (plain, _updates) = Vault::open(fixture.vault.path()).unwrap();
    let again = fixture.open();
    assert_eq!(*again.links(), *plain.links());
    assert_eq!(*again.tags(), *plain.tags());
}

/// Un link nuevo a una nota que no se tocó: la resolución no se guarda.
#[test]
fn los_links_de_una_nota_sin_cambios_se_resuelven_otra_vez() {
    let fixture = Fixture::new(&[("a.md", "[[b]]")]);
    fixture.open_and_close();
    assert_eq!(
        fixture.open().links().outgoing(&path("a.md"))[0].target,
        LinkTarget::Unresolved
    );

    fixture.write("b.md", "");
    let vault = fixture.open();
    assert_eq!(
        vault.links().outgoing(&path("a.md"))[0].target,
        LinkTarget::Resolved(path("b.md"))
    );
}

/// `TagPage` sale del texto, así que una página siempre se lee.
#[test]
fn una_pagina_de_tag_siempre_se_lee() {
    let fixture = Fixture::new(&[("tags/dos.md", "---\ncolor: red\n---\n")]);
    fixture.open_and_close();

    let stamp = fs::metadata(fixture.at("tags/dos.md"))
        .unwrap()
        .modified()
        .unwrap();
    fs::write(fixture.at("tags/dos.md"), "---\nicon: abc\n---\n").unwrap();
    File::options()
        .write(true)
        .open(fixture.at("tags/dos.md"))
        .unwrap()
        .set_modified(stamp)
        .unwrap();

    let vault = fixture.open();
    let tags = vault.tags();
    let page = tags.graph().page(&TagKey::new("dos")).unwrap();
    assert_eq!(page.color, None);
}

#[test]
fn una_carpeta_de_tags_distinta_no_estorba() {
    let fixture = Fixture::new(&[("otras/dos.md", "---\ncolor: red\n---\n#z")]);
    fixture.open_and_close();
    let other = VaultOptions {
        tags_folder: path("otras"),
        ..fixture.options()
    };
    let (vault, _updates) = Vault::open_with(fixture.vault.path(), other).unwrap();
    let tags = vault.tags();
    assert!(tags.graph().page(&TagKey::new("dos")).is_some());
    assert!(tags.index().tag(&TagKey::new("z")).is_none());
}

#[test]
fn una_cache_danada_no_cambia_nada_y_se_rehace() {
    let fixture = Fixture::new(NOTES);
    let (plain, _updates) = Vault::open(fixture.vault.path()).unwrap();
    fixture.open_and_close();
    let good = fs::read(fixture.cache_file()).unwrap();

    let mut flipped = good.clone();
    let middle = flipped.len() / 2;
    flipped[middle] ^= 1;
    let damaged: [&[u8]; 4] = [&flipped, &good[..good.len() / 2], b"", b"otra cosa"];
    for bytes in damaged {
        fs::write(fixture.cache_file(), bytes).unwrap();
        let vault = fixture.open();
        assert_eq!(*vault.links(), *plain.links());
        assert_eq!(*vault.tags(), *plain.tags());
        drop(vault);
        assert_eq!(fs::read(fixture.cache_file()).unwrap(), good, "se rehízo");
    }
}

#[test]
fn sin_cache_configurada_no_se_escribe_nada() {
    let fixture = Fixture::new(NOTES);
    let (vault, _updates) = Vault::open(fixture.vault.path()).unwrap();
    assert_eq!(targets(&vault.links(), "sub/c.md"), ["a", "d"]);
    drop(vault);
    assert!(!fixture.cache_file().exists());
}

#[test]
fn abrir_sin_poder_escribir_la_cache_funciona_igual() {
    let fixture = Fixture::new(NOTES);
    let blocked = fixture.data.path().join("archivo");
    fs::write(&blocked, "").unwrap();
    let options = VaultOptions {
        cache: Some(blocked.join("notes")),
        ..VaultOptions::default()
    };
    let (vault, _updates) = Vault::open_with(fixture.vault.path(), options).unwrap();
    assert_eq!(targets(&vault.links(), "sub/c.md"), ["a", "d"]);
}
