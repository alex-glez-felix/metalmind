//! Recorrer el disco con `ignore`. El walk completo y el parcial del watcher
//! usan las mismas reglas, así que reindexar a pedazos da lo mismo que de cero.

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read as _};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use ignore::{DirEntry, WalkBuilder, WalkState};

use crate::index::{EntryKind, VaultIndex};
use crate::path::{VaultPath, is_hidden_name};

const NODE_MODULES: &str = "node_modules";
const CACHEDIR_TAG: &str = "CACHEDIR.TAG";
const CACHEDIR_SIGNATURE: &[u8; 43] = b"Signature: 8a477f597d28d172789f06886806bc55";

/// Recorre en paralelo las notas (`.md`) de `root` con las reglas del índice,
/// para quien lea el disco por su cuenta (la búsqueda de texto). Cada hilo
/// arma su visitante con `visitor`, que recibe la ruta en el vault y la del
/// disco; un `Break` corta el walk en todos los hilos.
pub fn walk_notes<'s, V>(root: &'s Path, mut visitor: impl FnMut() -> V)
where
    V: FnMut(VaultPath, &Path) -> ControlFlow<()> + Send + 's,
{
    walker(root).build_parallel().run(|| {
        let mut visit = visitor();
        Box::new(move |result| {
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    log::warn!("al recorrer el vault: {error}");
                    return WalkState::Continue;
                }
            };
            match classify(root, &entry) {
                Some((path, EntryKind::Note)) => match visit(path, entry.path()) {
                    ControlFlow::Continue(()) => WalkState::Continue,
                    ControlFlow::Break(()) => WalkState::Quit,
                },
                _ => WalkState::Continue,
            }
        })
    });
}

fn walker(root: &Path) -> WalkBuilder {
    let mut builder = rules(root);
    builder.filter_entry(in_vault);
    builder
}

/// Las reglas del vault, iguales en cualquier máquina: `.gitignore` e
/// `.ignore` del vault aunque no sea un repo git, pero no los de carpetas
/// superiores ni el `excludesFile` global del usuario. Los ocultos van fuera
/// por un filtro propio, porque un `!.obsidian/` en `.gitignore` le gana a
/// `hidden(true)`.
fn rules(root: &Path) -> WalkBuilder {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .parents(false)
        .ignore(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .follow_links(false);
    builder
}

/// Lo que nunca es del vault aunque no lo ignore ningún `.gitignore`: lo
/// oculto, las dependencias de Node y las carpetas de caché.
fn in_vault(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_str();
    if name.is_some_and(is_hidden_name) {
        return false;
    }
    let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
    !(is_dir && (name == Some(NODE_MODULES) || is_cache_dir(entry.path())))
}

/// Ver <https://bford.info/cachedir/>: el `target` de cargo lo lleva.
fn is_cache_dir(dir: &Path) -> bool {
    let mut signature = [0; CACHEDIR_SIGNATURE.len()];
    File::open(dir.join(CACHEDIR_TAG))
        .and_then(|mut tag| tag.read_exact(&mut signature))
        .is_ok_and(|()| signature == *CACHEDIR_SIGNATURE)
}

/// La ruta y el tipo de una entrada del walk, como los guarda el índice: nada
/// para la raíz, los enlaces simbólicos (podrían sacar al vault de su raíz) y
/// lo que no es una ruta válida del vault.
fn classify(root: &Path, entry: &DirEntry) -> Option<(VaultPath, EntryKind)> {
    if entry.depth() == 0 {
        return None;
    }
    let file_type = entry.file_type()?;
    let Some(path) = VaultPath::from_fs(root, entry.path()) else {
        log::warn!("{} no es una ruta válida del vault", entry.path().display());
        return None;
    };
    if file_type.is_dir() {
        Some((path, EntryKind::Folder))
    } else if file_type.is_file() {
        let kind = EntryKind::of_file(&path);
        Some((path, kind))
    } else {
        None
    }
}

fn accept(root: &Path, result: Result<DirEntry, ignore::Error>) -> Option<(VaultPath, EntryKind)> {
    match result {
        Ok(entry) => classify(root, &entry),
        Err(error) => {
            log::warn!("al recorrer el vault: {error}");
            None
        }
    }
}

/// El índice de todo `root`, recorrido en paralelo.
pub(crate) fn scan(root: &Path) -> VaultIndex {
    let (sender, receiver) = mpsc::channel();
    walker(root).build_parallel().run(|| {
        let sender = sender.clone();
        let root = root.to_owned();
        Box::new(move |result| {
            if let Some(found) = accept(&root, result) {
                // Solo falla si el receptor ya no existe, y vive hasta el final del walk.
                let _ = sender.send(found);
            }
            WalkState::Continue
        })
    });
    drop(sender);
    VaultIndex::from_entries(receiver)
}

/// Lo que el walk parcial encontró, y las carpetas que no pudo leer: de esas
/// no se sabe qué hay, así que el índice conserva lo que tenía.
#[derive(Debug, Default)]
pub(crate) struct Partial {
    pub entries: Vec<(VaultPath, EntryKind)>,
    pub unreadable: Vec<VaultPath>,
}

/// Lo que hay hoy en disco en `dirty` y en sus carpetas. Recorre desde la raíz
/// para que apliquen todos los `.gitignore` de por medio, pero solo entra en
/// los ancestros de `dirty`.
pub(crate) fn scan_partial(root: &Path, dirty: &[VaultPath]) -> Partial {
    let scope = Arc::new(Scope::new(dirty));
    let root_owned: PathBuf = root.to_owned();
    let filter_scope = Arc::clone(&scope);
    let mut found = Partial::default();
    let walk = rules(root)
        .filter_entry(move |entry| {
            in_vault(entry)
                && VaultPath::from_fs(&root_owned, entry.path())
                    .is_some_and(|path| filter_scope.allows(&path))
        })
        .build();
    for result in walk {
        match result {
            Ok(entry) => {
                if let Some((path, kind)) = classify(root, &entry)
                    && scope.contains(&path)
                {
                    found.entries.push((path, kind));
                }
            }
            Err(error) => {
                if let Some(path) = unreadable(root, &error) {
                    log::warn!("al releer el vault: {error}");
                    found.unreadable.push(path);
                }
            }
        }
    }
    found
}

/// Qué parte del vault quedó sin leer por `error`: nada si lo que faltaba ya
/// no existe (se borró mientras se recorría), todo si no dice dónde fue.
fn unreadable(root: &Path, error: &ignore::Error) -> Option<VaultPath> {
    if error
        .io_error()
        .is_some_and(|io| io.kind() == io::ErrorKind::NotFound)
    {
        return None;
    }
    Some(
        error_path(error)
            .and_then(|path| VaultPath::from_fs(root, path))
            .unwrap_or_else(VaultPath::root),
    )
}

fn error_path(error: &ignore::Error) -> Option<&Path> {
    match error {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::Loop { child, .. } => Some(child),
        ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
            error_path(err)
        }
        ignore::Error::Partial(errors) => errors.first().and_then(error_path),
        ignore::Error::Io(_)
        | ignore::Error::Glob { .. }
        | ignore::Error::UnrecognizedFileType(_)
        | ignore::Error::InvalidDefinition => None,
    }
}

/// Las rutas sucias y sus ancestros: lo que el walk parcial tiene que pisar.
struct Scope {
    dirty: HashSet<VaultPath>,
    ancestors: HashSet<VaultPath>,
}

impl Scope {
    fn new(dirty: &[VaultPath]) -> Self {
        Self {
            dirty: dirty.iter().cloned().collect(),
            ancestors: dirty.iter().flat_map(VaultPath::ancestors).collect(),
        }
    }

    /// Si `path` es sucia o está dentro de una sucia.
    fn contains(&self, path: &VaultPath) -> bool {
        self.dirty.contains(path)
            || path
                .ancestors()
                .any(|ancestor| self.dirty.contains(&ancestor))
    }

    fn allows(&self, path: &VaultPath) -> bool {
        self.ancestors.contains(path) || self.contains(path)
    }
}
