#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use std::fs;
use std::fs::Permissions;
use std::os::unix::fs::{PermissionsExt as _, symlink};

use metalmind_vault::{EntryKind, Vault, VaultError, VaultPath, VaultRoot};
use pretty_assertions::assert_eq;
use support::{listing, make_tree};
use tempfile::TempDir;

#[test]
fn indexa_notas_adjuntos_y_carpetas() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &["a.md", "img.png", "sub/b.md", "sub/hondo/c.MD", "vacia/"],
    );
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(
        listing(&vault.snapshot()),
        [
            "sub/",
            "sub/hondo/",
            "sub/hondo/c.MD (nota)",
            "sub/b.md (nota)",
            "vacia/",
            "a.md (nota)",
            "img.png (adjunto)",
        ]
    );
}

#[test]
fn ignora_siempre_obsidian_git_trash_y_ocultos() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &[
            "a.md",
            ".obsidian/app.json",
            ".git/HEAD",
            ".trash/vieja.md",
            ".oculta.md",
            "sub/.borrador/x.md",
        ],
    );
    // Ni volviéndolas a incluir desde .gitignore.
    fs::write(dir.path().join(".gitignore"), "!.obsidian/\n!.oculta.md\n").unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(listing(&vault.snapshot()), ["sub/", "a.md (nota)"]);
}

/// #97: ni `node_modules` ni lo que se marca como caché (el `target` de
/// cargo) son notas, y vigilarlos gasta watches de inotify.
#[test]
fn ignora_node_modules_y_las_carpetas_de_cache() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &[
            "a.md",
            "node_modules/react/README.md",
            "app/node_modules/x/y.md",
            "target/debug/build/x.md",
            "cache/z.md",
            "target-sin-marca/target/b.md",
            "otro/CACHEDIR.TAG",
        ],
    );
    let signature = "Signature: 8a477f597d28d172789f06886806bc55\n# cargo\n";
    fs::write(dir.path().join("target/CACHEDIR.TAG"), signature).unwrap();
    fs::write(dir.path().join("cache/CACHEDIR.TAG"), signature).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(
        listing(&vault.snapshot()),
        [
            "app/",
            "otro/",
            "otro/CACHEDIR.TAG (adjunto)",
            "target-sin-marca/",
            "target-sin-marca/target/",
            "target-sin-marca/target/b.md (nota)",
            "a.md (nota)",
        ],
        "sin la firma, CACHEDIR.TAG es un archivo más"
    );
}

#[test]
fn respeta_gitignore_e_ignore_aunque_no_sea_un_repo() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &[
            "a.md",
            "build/x.md",
            "sub/b.md",
            "sub/c.log",
            "privado/d.md",
        ],
    );
    fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
    fs::write(dir.path().join("sub/.gitignore"), "*.log\n").unwrap();
    fs::write(dir.path().join(".ignore"), "privado/\n").unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(
        listing(&vault.snapshot()),
        ["sub/", "sub/b.md (nota)", "a.md (nota)"]
    );
}

#[test]
fn no_lee_ignores_de_fuera_del_vault() {
    let outer = TempDir::new().unwrap();
    fs::write(outer.path().join(".gitignore"), "*.md\n").unwrap();
    make_tree(outer.path(), &["vault/a.md"]);
    let (vault, _updates) = Vault::open(outer.path().join("vault")).unwrap();
    assert_eq!(listing(&vault.snapshot()), ["a.md (nota)"]);
}

#[test]
fn no_sigue_ni_lista_enlaces_simbolicos() {
    let outside = TempDir::new().unwrap();
    make_tree(outside.path(), &["secreto.md", "carpeta/x.md"]);
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md"]);
    symlink(
        outside.path().join("secreto.md"),
        dir.path().join("enlace.md"),
    )
    .unwrap();
    symlink(outside.path().join("carpeta"), dir.path().join("enlace")).unwrap();
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(listing(&vault.snapshot()), ["a.md (nota)"]);
}

#[test]
fn la_raiz_puede_ser_un_enlace() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["real/a.md"]);
    symlink(dir.path().join("real"), dir.path().join("enlace")).unwrap();
    let (vault, _updates) = Vault::open(dir.path().join("enlace")).unwrap();
    assert_eq!(
        vault.root(),
        dir.path().join("real").canonicalize().unwrap()
    );
    assert_eq!(listing(&vault.snapshot()), ["a.md (nota)"]);
}

#[test]
fn abrir_algo_que_no_es_carpeta_falla() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md"]);
    assert!(matches!(
        Vault::open(dir.path().join("a.md")),
        Err(VaultError::NotADirectory(_))
    ));
    assert!(matches!(
        Vault::open(dir.path().join("no-existe")),
        Err(VaultError::Open { .. })
    ));
}

#[test]
fn el_arbol_se_recorre_desde_la_raiz() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["b/x.md", "a.md"]);
    let index = Vault::open(dir.path()).unwrap().0.snapshot();
    let root: Vec<_> = index
        .children(&VaultPath::root())
        .map(|entry| (entry.path.as_str(), entry.kind))
        .collect();
    assert_eq!(root, [("b", EntryKind::Folder), ("a.md", EntryKind::Note)]);
    let b = VaultPath::new("b").unwrap();
    assert_eq!(index.children(&b).count(), 1);
    assert_eq!(index.files().count(), 2);
    assert_eq!(
        index.kind(&VaultPath::new("b/x.md").unwrap()),
        Some(EntryKind::Note)
    );
}

#[test]
fn la_raiz_sola_lista_lo_mismo_que_open() {
    let dir = TempDir::new().unwrap();
    make_tree(dir.path(), &["a.md", "sub/b.md", "img.png", ".oculta.md"]);
    let root = VaultRoot::new(dir.path()).unwrap();
    let (opened, _updates) = Vault::open(dir.path()).unwrap();
    assert_eq!(root.path(), opened.root());
    assert_eq!(listing(&root.files()), listing(&opened.snapshot()));
}

/// Solo esas carpetas, con las reglas de todo el vault (el `.gitignore` de la
/// raíz también vale dentro) y las carpetas de arriba que las cuelgan.
#[test]
fn files_within_lee_solo_esas_carpetas() {
    let dir = TempDir::new().unwrap();
    make_tree(
        dir.path(),
        &[
            "meta/plantillas/a.md",
            "meta/plantillas/sub/b.md",
            "meta/plantillas/borrador.md",
            "meta/otra.md",
            "tags/rust.md",
            "notas/c.md",
            "plantillas.md",
        ],
    );
    fs::write(dir.path().join(".gitignore"), "borrador.md\n").unwrap();
    let root = VaultRoot::new(dir.path()).unwrap();
    let folders =
        ["meta/plantillas", "tags", "no/existe"].map(|path| VaultPath::new(path).unwrap());
    assert_eq!(
        listing(&root.files_within(&folders)),
        [
            "meta/",
            "meta/plantillas/",
            "meta/plantillas/sub/",
            "meta/plantillas/sub/b.md (nota)",
            "meta/plantillas/a.md (nota)",
            "tags/",
            "tags/rust.md (nota)",
        ]
    );
}

#[test]
fn una_raiz_que_no_se_puede_leer_no_es_un_vault_vacio() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("cerrada");
    make_tree(dir.path(), &["cerrada/a.md"]);
    fs::set_permissions(&root, Permissions::from_mode(0o300)).unwrap();
    let loaded = VaultRoot::new(&root);
    let opened = Vault::open(&root).map(|(vault, _)| vault);
    fs::set_permissions(&root, Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(loaded, Err(VaultError::Open { .. })));
    assert!(matches!(opened, Err(VaultError::Open { .. })));
}
