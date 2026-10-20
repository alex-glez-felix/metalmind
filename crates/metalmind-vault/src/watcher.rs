//! Qué vigila un vault abierto: la raíz y cada carpeta del índice, una por
//! una y sin recursión. Así lo que el vault ignora (`node_modules`, `.git`,
//! lo de un `.gitignore`) no gasta watches de inotify. El walk vigila cada
//! carpeta antes de leerla, así que lo que cambie después da evento.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use notify::{ErrorKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{Debouncer, RecommendedCache};

use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;
use crate::scan::BeforeReading;

/// Las carpetas de git que se vigilan aunque estén ocultas: tocar
/// `.git/info/exclude` relee el vault, y `.git/info` puede aparecer después.
const GIT_DIRS: [&str; 2] = [".git", ".git/info"];

/// Por qué un vault abierto no ve los cambios que se hacen fuera de Metalmind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unwatched {
    #[error("se llegó al límite de watches de inotify (fs.inotify.max_user_watches)")]
    WatchLimit,
}

/// Lo que se necesita del watcher de `notify`, para poder probarlo sin inotify.
pub(crate) trait Watches {
    fn watch(&mut self, dir: &Path) -> notify::Result<()>;
    fn unwatch(&mut self, dir: &Path) -> notify::Result<()>;
    /// Deja de vigilar y espera al hilo que entrega los eventos.
    fn stop(self);
}

impl Watches for Debouncer<RecommendedWatcher, RecommendedCache> {
    fn watch(&mut self, dir: &Path) -> notify::Result<()> {
        Self::watch(self, dir, RecursiveMode::NonRecursive)
    }

    fn unwatch(&mut self, dir: &Path) -> notify::Result<()> {
        Self::unwatch(self, dir)
    }

    fn stop(self) {
        Self::stop(self);
    }
}

/// Lo que vigila un vault abierto.
pub(crate) type Watch = FolderWatch<Debouncer<RecommendedWatcher, RecommendedCache>>;

pub(crate) struct FolderWatch<W> {
    root: PathBuf,
    /// `None` desde que se perdió la vigilancia, que no vuelve.
    watches: Option<W>,
    watched: HashSet<PathBuf>,
    lost: Arc<OnceLock<Unwatched>>,
}

impl<W: Watches> FolderWatch<W> {
    /// Vigila `root` y sus carpetas de git, antes del walk. Si no quedan
    /// watches, el vault abre igual, sin vigilancia, y `lost` dice por qué.
    ///
    /// # Errors
    /// Si `root` no se puede vigilar por otra razón.
    pub(crate) fn start(
        root: &Path,
        mut watches: W,
        lost: Arc<OnceLock<Unwatched>>,
    ) -> notify::Result<Self> {
        if let Err(error) = watches.watch(root) {
            if !matches!(error.kind, ErrorKind::MaxFilesWatch) {
                return Err(error);
            }
            give_up(&lost, root, watches);
            return Ok(Self {
                root: root.to_owned(),
                watches: None,
                watched: HashSet::new(),
                lost,
            });
        }
        let mut watch = Self {
            root: root.to_owned(),
            watches: Some(watches),
            watched: HashSet::from([root.to_owned()]),
            lost,
        };
        watch.git_dirs();
        Ok(watch)
    }

    /// Vigila la carpeta `dir` si no lo está ya. El walk la pasa antes de
    /// leerla.
    pub(crate) fn folder(&mut self, dir: &Path) {
        let Some(watches) = &mut self.watches else {
            return;
        };
        if self.watched.contains(dir) {
            return;
        }
        match watches.watch(dir) {
            Ok(()) => {
                self.watched.insert(dir.to_owned());
            }
            Err(error) if matches!(error.kind, ErrorKind::MaxFilesWatch) => self.lose(),
            // Se borró mientras se recorría: su carpeta ya avisó.
            Err(error) if is_gone(&error) => {}
            Err(error) => log::warn!("no se vigila {}: {error}", dir.display()),
        }
    }

    /// Después de releer: suelta las carpetas que salieron del índice y
    /// vigila las de git que aparecieron. `true` si hay que releer todo el
    /// vault, porque un `.git/info/exclude` de antes no dio evento.
    pub(crate) fn follow(&mut self, index: &VaultIndex) -> bool {
        let Some(watches) = &mut self.watches else {
            return false;
        };
        let root = &self.root;
        let left: Vec<PathBuf> = self
            .watched
            .iter()
            .filter(|dir| !is_git_dir(root, dir) && !is_folder_in(root, dir, index))
            .cloned()
            .collect();
        for dir in left {
            self.watched.remove(&dir);
            // Una carpeta movida conserva su watch, y el walk ya la vigiló en
            // su ruta nueva: soltar la vieja la dejaría sin ninguno. Solo se
            // suelta lo que sigue ahí, como lo que pasó a estar ignorado.
            if is_dir(&dir) {
                // Si ya no tenía watch, no hay nada que soltar.
                let _ = watches.unwatch(&dir);
            }
        }
        self.watched
            .retain(|dir| !is_git_dir(root, dir) || is_dir(dir));
        self.git_dirs()
    }

    /// Si se perdió la vigilancia.
    pub(crate) fn lost(&self) -> Option<Unwatched> {
        self.lost.get().copied()
    }

    pub(crate) fn stop(&mut self) {
        if let Some(watches) = self.watches.take() {
            watches.stop();
        }
    }

    /// Vigila las carpetas de git que existen y no se vigilaban; `true` si
    /// vigiló alguna.
    fn git_dirs(&mut self) -> bool {
        let mut new = false;
        for dir in GIT_DIRS.map(|dir| self.root.join(dir)) {
            if !self.watched.contains(&dir) && is_dir(&dir) {
                self.folder(&dir);
                new |= self.watched.contains(&dir);
            }
        }
        new
    }

    /// Con unos pocos watches menos, un cambio dentro de esas carpetas se
    /// perdería en silencio: mejor no vigilar nada y que se sepa.
    fn lose(&mut self) {
        if let Some(watches) = self.watches.take() {
            give_up(&self.lost, &self.root, watches);
        }
        self.watched.clear();
    }
}

/// Para el walk: cada carpeta que va a leer se vigila antes.
pub(crate) fn before_reading<W: Watches + Send + 'static>(
    watch: &Arc<Mutex<FolderWatch<W>>>,
) -> BeforeReading {
    let watch = Arc::clone(watch);
    Arc::new(move |dir| {
        watch
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .folder(dir);
    })
}

fn give_up<W: Watches>(lost: &OnceLock<Unwatched>, root: &Path, watches: W) {
    watches.stop();
    let reason = *lost.get_or_init(|| Unwatched::WatchLimit);
    log::warn!(
        "{reason}: Metalmind no verá los cambios que se hagan en {} fuera de él. \
         Para subirlo: `sudo sysctl fs.inotify.max_user_watches=524288`, y para \
         que dure: `echo fs.inotify.max_user_watches=524288 | sudo tee \
         /etc/sysctl.d/90-inotify.conf`",
        root.display()
    );
}

fn is_git_dir(root: &Path, dir: &Path) -> bool {
    GIT_DIRS.iter().any(|git| dir == root.join(git))
}

fn is_folder_in(root: &Path, dir: &Path, index: &VaultIndex) -> bool {
    VaultPath::from_fs(root, dir)
        .is_some_and(|path| path.is_root() || index.kind(&path) == Some(EntryKind::Folder))
}

/// Sin seguir enlaces: el vault no los sigue.
fn is_dir(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|meta| meta.is_dir())
}

fn is_gone(error: &notify::Error) -> bool {
    match &error.kind {
        ErrorKind::PathNotFound => true,
        ErrorKind::Io(error) => error.kind() == io::ErrorKind::NotFound,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::fs;
    use std::rc::Rc;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    /// Un watcher de mentira que anota lo que vigila y se queda sin watches
    /// después de `limit`.
    #[derive(Clone)]
    struct Fake {
        watched: Rc<RefCell<BTreeSet<PathBuf>>>,
        log: Rc<RefCell<Vec<String>>>,
        limit: usize,
    }

    impl Fake {
        fn new(limit: usize) -> Self {
            Self {
                watched: Rc::default(),
                log: Rc::default(),
                limit,
            }
        }

        fn relative(&self, root: &Path) -> Vec<String> {
            let watched = self.watched.borrow();
            watched
                .iter()
                .map(|dir| dir.strip_prefix(root).unwrap().display().to_string())
                .collect()
        }
    }

    impl Watches for Fake {
        fn watch(&mut self, dir: &Path) -> notify::Result<()> {
            let mut watched = self.watched.borrow_mut();
            if watched.len() == self.limit {
                return Err(notify::Error::new(ErrorKind::MaxFilesWatch));
            }
            self.log.borrow_mut().push(format!("+{}", dir.display()));
            watched.insert(dir.to_owned());
            Ok(())
        }

        fn unwatch(&mut self, dir: &Path) -> notify::Result<()> {
            self.log.borrow_mut().push(format!("-{}", dir.display()));
            self.watched.borrow_mut().remove(dir);
            Ok(())
        }

        fn stop(self) {
            self.watched.borrow_mut().clear();
        }
    }

    fn folders(paths: &[&str]) -> VaultIndex {
        VaultIndex::from_entries(
            paths
                .iter()
                .map(|path| (VaultPath::new(path).unwrap(), EntryKind::Folder)),
        )
    }

    fn start(root: &Path, fake: &Fake) -> FolderWatch<Fake> {
        FolderWatch::start(root, fake.clone(), Arc::default()).unwrap()
    }

    #[test]
    fn vigila_cada_carpeta_una_vez() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let fake = Fake::new(usize::MAX);
        let mut watch = start(root, &fake);
        assert_eq!(fake.relative(root), [""]);
        watch.folder(&root.join("a"));
        watch.folder(&root.join("a/b"));
        watch.folder(&root.join("a"));
        assert_eq!(fake.relative(root), ["", "a", "a/b"]);
        assert_eq!(fake.log.borrow().len(), 3);
        assert_eq!(watch.lost(), None);
    }

    #[test]
    fn suelta_lo_que_sale_del_indice_pero_no_lo_movido() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("ignorada")).unwrap();
        let fake = Fake::new(usize::MAX);
        let mut watch = start(root, &fake);
        for name in ["ignorada", "movida", "nueva"] {
            watch.folder(&root.join(name));
        }
        fake.log.borrow_mut().clear();

        // `movida` ya no existe: su watch sigue a la carpeta, que el walk
        // vigiló en `nueva`.
        assert!(!watch.follow(&folders(&["nueva"])));
        let ignored = format!("-{}", root.join("ignorada").display());
        assert_eq!(*fake.log.borrow(), [ignored]);
        watch.folder(&root.join("movida"));
        assert_eq!(
            fake.log.borrow().last(),
            Some(&format!("+{}", root.join("movida").display())),
            "si vuelve a aparecer, se vigila otra vez"
        );
    }

    #[test]
    fn vigila_git_aunque_este_oculto_y_si_aparece_relee_todo() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".git")).unwrap();
        let fake = Fake::new(usize::MAX);
        let mut watch = start(root, &fake);
        assert_eq!(fake.relative(root), ["", ".git"]);
        assert!(!watch.follow(&folders(&[])));
        fs::create_dir(root.join(".git/info")).unwrap();
        assert!(watch.follow(&folders(&[])));
        assert_eq!(fake.relative(root), ["", ".git", ".git/info"]);
        assert!(!watch.follow(&folders(&[])));
    }

    #[test]
    fn sin_watches_deja_de_vigilar_todo_y_dice_por_que() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let fake = Fake::new(2);
        let mut watch = start(root, &fake);
        for name in ["a", "b", "c"] {
            watch.folder(&root.join(name));
        }
        assert_eq!(watch.lost(), Some(Unwatched::WatchLimit));
        assert_eq!(
            fake.relative(root),
            Vec::<String>::new(),
            "no queda ninguno"
        );
        watch.folder(&root.join("d"));
        assert!(!watch.follow(&folders(&["a", "b", "c", "d"])));
        assert_eq!(fake.relative(root), Vec::<String>::new());
    }

    #[test]
    fn sin_watches_ni_para_la_raiz_el_vault_abre_igual() {
        let dir = TempDir::new().unwrap();
        let lost = Arc::default();
        let mut watch = FolderWatch::start(dir.path(), Fake::new(0), Arc::clone(&lost)).unwrap();
        assert_eq!(lost.get(), Some(&Unwatched::WatchLimit));
        watch.folder(&dir.path().join("a"));
        assert_eq!(watch.lost(), Some(Unwatched::WatchLimit));
    }
}
