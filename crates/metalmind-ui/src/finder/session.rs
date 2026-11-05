//! El vault de la ventana mientras está abierta: el índice que mantiene el
//! watcher y las notas recientes, guardadas en el directorio de estado del
//! vault. Lo usan el finder y el panel lateral: cada lote del watcher llega
//! al hilo de la UI por un canal y sale como [`VaultChanged`]. Guardar los
//! recientes pasa en un hilo propio ([`RecentsWriter`]).

use std::io;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::{fs, thread};

use gpui_kit::{Context, EventEmitter, Subscription, Task};
use metalmind_config::{Paths, SavedView};
use metalmind_vault::tags::Tags;
use metalmind_vault::{Unwatched, Vault, VaultIndex, VaultPath, VaultUpdate};

use super::writer::RecentsWriter;
use super::{Note, Recents, notes};
use crate::focus::{Focus, InScope};

/// El archivo de los recientes dentro del estado del vault.
const RECENTS_FILE: &str = "recientes";

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("no se pudo arrancar el hilo que sigue al vault: {0}")]
    Thread(io::Error),
}

/// Un lote del watcher, ya aplicado al índice.
pub struct VaultChanged(pub VaultUpdate);

/// El vault dejó de ver los cambios de fuera mientras estaba abierto.
pub struct WatchLost(pub Unwatched);

/// Se puso, cambió o se quitó el focus.
pub struct FocusChanged;

/// Las vistas guardadas del vault cambiaron; `renamed`, si fue porque una
/// cambió de nombre (de, a), para que lo que la mostraba la siga.
pub struct ViewsChanged {
    pub renamed: Option<(String, String)>,
}

pub struct VaultSession {
    /// Compartido con el panel lateral, que hace las operaciones de archivo.
    vault: Rc<Vault>,
    recents: Recents,
    /// `None` si no hay directorio de estado: se recuerdan solo en memoria.
    writer: Option<RecentsWriter>,
    /// Las notas del finder, solo las del focus si hay uno.
    notes: Arc<[Note]>,
    tags: Arc<Tags>,
    /// El focus de la sesión (05 §3): al reabrir, la app empieza sin él.
    focus: Option<(Focus, Arc<InScope>)>,
    /// Las vistas guardadas (06 §10), como se leyeron la última vez.
    views: Arc<[SavedView]>,
    unwatched: Option<Unwatched>,
    _updates: Task<()>,
    _quit: Subscription,
}

impl EventEmitter<VaultChanged> for VaultSession {}
impl EventEmitter<WatchLost> for VaultSession {}
impl EventEmitter<FocusChanged> for VaultSession {}
impl EventEmitter<ViewsChanged> for VaultSession {}

/// Lo que se prepara antes de abrir la ventana: los hilos que siguen al
/// vault y guardan los recientes, y los recientes guardados.
pub struct Prepared {
    vault: Vault,
    updates: async_channel::Receiver<VaultUpdate>,
    recents: Recents,
    writer: Option<RecentsWriter>,
}

impl Prepared {
    /// # Errors
    /// Si no se puede arrancar alguno de los dos hilos.
    pub fn new(
        vault: Vault,
        updates: mpsc::Receiver<VaultUpdate>,
        paths: Option<&Paths>,
    ) -> Result<Self, SessionError> {
        let file = paths.map(|paths| paths.vault_state(vault.root()).join(RECENTS_FILE));
        let recents = file.as_deref().map(read_recents).unwrap_or_default();
        let writer = file
            .map(RecentsWriter::spawn)
            .transpose()
            .map_err(SessionError::Thread)?;
        let (sender, receiver) = async_channel::unbounded();
        forward(updates, sender).map_err(SessionError::Thread)?;
        Ok(Self {
            vault,
            updates: receiver,
            recents,
            writer,
        })
    }
}

#[cfg(test)]
impl Prepared {
    /// Sin el hilo que trae los lotes del watcher: en un test, que despierte
    /// la prueba desde otro hilo lo rechaza GPUI.
    pub fn without_updates(vault: Vault) -> Self {
        let (_, updates) = async_channel::unbounded();
        Self {
            vault,
            updates,
            recents: Recents::default(),
            writer: None,
        }
    }
}

impl VaultSession {
    pub fn new(prepared: Prepared, cx: &mut Context<Self>) -> Self {
        let Prepared {
            vault,
            updates,
            recents,
            writer,
        } = prepared;
        let updates = cx.spawn(async move |this, cx| {
            while let Ok(update) = updates.recv().await {
                let applied = this.update(cx, |this: &mut Self, cx| this.apply(update, cx));
                if applied.is_err() {
                    return;
                }
            }
        });
        // Al salir de la app se espera a la última escritura de los recientes
        // y a las versiones de los últimos guardados.
        let quit = cx.on_app_quit(|this: &mut Self, _| {
            this.writer.take();
            this.vault.shut_down();
            async {}
        });
        let notes = notes(&vault.snapshot(), &recents);
        let tags = vault.tags();
        let unwatched = vault.unwatched();
        // Con la config rota, ninguna: quien la lee al abrir lo dice.
        let (views, _) = crate::note_list::load_views(vault.root());
        Self {
            vault: Rc::new(vault),
            recents,
            writer,
            notes,
            tags,
            focus: None,
            views: views.into(),
            unwatched,
            _updates: updates,
            _quit: quit,
        }
    }

    /// El vault, para quien hace operaciones de archivo sobre él.
    #[must_use]
    pub fn vault(&self) -> Rc<Vault> {
        Rc::clone(&self.vault)
    }

    /// La raíz, sin enlaces simbólicos.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.vault.root()
    }

    /// Por qué no se ven los cambios de fuera, si no se ven.
    #[must_use]
    pub fn unwatched(&self) -> Option<Unwatched> {
        self.unwatched
    }

    /// Lo que lista el finder ahora: con focus, solo lo de su alcance.
    #[must_use]
    pub fn notes(&self) -> Arc<[Note]> {
        Arc::clone(&self.notes)
    }

    #[must_use]
    pub fn focus(&self) -> Option<&Focus> {
        self.focus.as_ref().map(|(focus, _)| focus)
    }

    /// Lo que queda dentro del focus con el índice de ahora.
    #[must_use]
    pub fn in_scope(&self) -> Option<Arc<InScope>> {
        self.focus
            .as_ref()
            .map(|(_, in_scope)| Arc::clone(in_scope))
    }

    /// Pone o quita el focus; el finder, el panel y la búsqueda lo siguen.
    pub fn set_focus(&mut self, focus: Option<Focus>, cx: &mut Context<Self>) {
        let index = self.vault.snapshot();
        self.focus = focus.map(|focus| {
            let in_scope = Arc::new(focus.resolve(&index, &self.tags));
            (focus, in_scope)
        });
        self.notes = self.list(&index);
        cx.emit(FocusChanged);
        cx.notify();
    }

    /// `F` en el panel: pone `focus`, o lo quita si ya es ese alcance.
    pub fn toggle_focus(&mut self, focus: Focus, cx: &mut Context<Self>) {
        let same = self
            .focus()
            .is_some_and(|current| current.same_scope(&focus));
        self.set_focus((!same).then_some(focus), cx);
    }

    #[must_use]
    pub fn views(&self) -> Arc<[SavedView]> {
        Arc::clone(&self.views)
    }

    /// Las vistas como quedaron en el archivo; el focus sigue a las suyas
    /// (ver [`Focus::follow_views`]). Si son las mismas, nadie se entera:
    /// el panel no rehace su árbol cada vez que se abre `space v`.
    pub fn set_views(
        &mut self,
        views: Vec<SavedView>,
        renamed: Option<(&str, &str)>,
        cx: &mut Context<Self>,
    ) {
        if renamed.is_none() && *self.views == *views {
            return;
        }
        self.views = views.into();
        let followed = self
            .focus
            .as_mut()
            .is_some_and(|(focus, _)| focus.follow_views(&self.views, renamed));
        if followed && let Some((focus, _)) = self.focus.take() {
            self.set_focus(Some(focus), cx);
        }
        cx.emit(ViewsChanged {
            renamed: renamed.map(|(from, to)| (from.to_owned(), to.to_owned())),
        });
        cx.notify();
    }

    /// Las notas del finder en `index`, dentro del focus si hay uno.
    fn list(&self, index: &VaultIndex) -> Arc<[Note]> {
        let notes = notes(index, &self.recents);
        match &self.focus {
            Some((_, in_scope)) => notes
                .iter()
                .filter(|note| in_scope.contains_file(&note.0))
                .cloned()
                .collect(),
            None => notes,
        }
    }

    /// Los tags del último lote del watcher.
    #[must_use]
    pub fn tags(&self) -> Arc<Tags> {
        Arc::clone(&self.tags)
    }

    /// La ruta del vault de `path`, si está dentro.
    #[must_use]
    pub fn vault_path(&self, path: &Path) -> Option<VaultPath> {
        relative_to(self.root(), &canonical(path)?)
    }

    /// `path` se acaba de abrir: pasa a ser la más reciente.
    pub fn touch(&mut self, path: VaultPath, cx: &mut Context<Self>) {
        if self.recents.touch(path) {
            self.recents_changed(cx);
        }
    }

    fn apply(&mut self, update: VaultUpdate, cx: &mut Context<Self>) {
        let mut moved = false;
        for change in &update.changes {
            moved |= self.recents.apply(change);
        }
        if moved {
            self.save();
        }
        self.tags = Arc::clone(&update.tags);
        let mut moved = false;
        if let Some((focus, in_scope)) = &mut self.focus {
            moved = focus.follow(&update.changes);
            *in_scope = Arc::new(focus.resolve(&update.snapshot, &self.tags));
        }
        self.notes = self.list(&update.snapshot);
        cx.emit(VaultChanged(update));
        if moved {
            cx.emit(FocusChanged);
        }
        // Se pierde al aparecer carpetas, así que llega con un lote.
        if let (None, Some(reason)) = (self.unwatched, self.vault.unwatched()) {
            self.unwatched = Some(reason);
            cx.emit(WatchLost(reason));
        }
        cx.notify();
    }

    fn recents_changed(&mut self, cx: &mut Context<Self>) {
        self.notes = self.list(&self.vault.snapshot());
        self.save();
        cx.notify();
    }

    fn save(&self) {
        if let Some(writer) = &self.writer {
            writer.write(self.recents.to_text());
        }
    }
}

/// Pasa los cambios del watcher, que llegan por un canal bloqueante, a uno
/// que el hilo de la UI puede esperar. El hilo termina cuando se suelta el
/// vault (se cierra `updates`) o la sesión (se cierra `sender`).
fn forward(
    updates: mpsc::Receiver<VaultUpdate>,
    sender: async_channel::Sender<VaultUpdate>,
) -> io::Result<()> {
    thread::Builder::new()
        .name("metalmind-ui vault".to_owned())
        .spawn(move || {
            for update in updates {
                if sender.send_blocking(update).is_err() {
                    return;
                }
            }
        })
        .map(drop)
}

/// `path`, absoluta y canónica, como ruta del vault de raíz `root`.
pub(crate) fn relative_to(root: &Path, path: &Path) -> Option<VaultPath> {
    let parts: Option<Vec<&str>> = path
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|component| match component {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    VaultPath::new(&parts?.join("/"))
        .ok()
        .filter(|path| !path.is_root())
}

/// La ruta sin enlaces simbólicos. Lo que todavía no existe (una nota nueva
/// sin guardar, o una cuya carpeta se borró o se movió desde fuera) se agrega
/// tal cual a lo último que sí existe; un `..` en esa parte no se resuelve.
pub(crate) fn canonical(path: &Path) -> Option<PathBuf> {
    let mut missing = Vec::new();
    let mut current = path;
    loop {
        let probe = if current.as_os_str().is_empty() {
            Path::new(".")
        } else {
            current
        };
        if let Ok(real) = fs::canonicalize(probe) {
            return Some(
                missing
                    .iter()
                    .rev()
                    .fold(real, |path, name| path.join(name)),
            );
        }
        missing.push(current.file_name()?);
        current = current.parent()?;
    }
}

fn read_recents(file: &Path) -> Recents {
    match metalmind_vault::read(file) {
        Ok(text) => Recents::parse(&text.unwrap_or_default()),
        Err(error) => {
            log::warn!("no se leyeron las notas recientes: {error}");
            Recents::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use gpui_kit::{AppContext as _, TestAppContext};

    use super::*;

    /// Las vistas se leen del vault al abrir; cuando cambian, el focus de
    /// una sigue a su nombre y su consulta nuevos, y se avisa de las dos
    /// cosas.
    #[test]
    fn el_focus_sigue_a_su_vista() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "#rust").unwrap();
        fs::write(dir.path().join("b.md"), "#gs").unwrap();
        fs::create_dir(dir.path().join(".metalmind")).unwrap();
        fs::write(
            dir.path().join(".metalmind/config.yaml"),
            "views:\n  gs: \"#gs\"\n",
        )
        .unwrap();
        let (vault, updates) = Vault::open(dir.path()).unwrap();
        let prepared = Prepared::new(vault, updates, None).unwrap();
        let mut cx = TestAppContext::single();
        let session = cx.update(|cx| cx.new(|cx| VaultSession::new(prepared, cx)));
        let views = session.read_with(&cx, |session, _| session.views());
        assert_eq!(views.len(), 1);
        let focus = Focus::parse("@gs", &views).unwrap();
        session.update(&mut cx, |session, cx| session.set_focus(focus, cx));
        let events = Rc::new(Cell::new((0, 0)));
        let (on_focus, on_views) = (Rc::clone(&events), Rc::clone(&events));
        cx.update(|cx| {
            cx.subscribe(&session, move |_, _: &FocusChanged, _| {
                let (focus, views) = on_focus.get();
                on_focus.set((focus + 1, views));
            })
            .detach();
            cx.subscribe(&session, move |_, _: &ViewsChanged, _| {
                let (focus, views) = on_views.get();
                on_views.set((focus, views + 1));
            })
            .detach();
        });
        let estudio = SavedView {
            name: "estudio".to_owned(),
            query: "#rust".to_owned(),
        };
        session.update(&mut cx, |session, cx| {
            session.set_views(vec![estudio], Some(("gs", "estudio")), cx);
        });
        cx.run_until_parked();
        assert_eq!(events.get(), (1, 1));
        session.read_with(&cx, |session, _| {
            assert_eq!(session.focus().unwrap().label(), "focus: @estudio");
            let in_scope = session.in_scope().unwrap();
            assert!(in_scope.contains_file(&VaultPath::new("a.md").unwrap()));
            assert!(!in_scope.contains_file(&VaultPath::new("b.md").unwrap()));
        });
        let same = session.read_with(&cx, |session, _| session.views().to_vec());
        session.update(&mut cx, |session, cx| session.set_views(same, None, cx));
        cx.run_until_parked();
        assert_eq!(events.get(), (1, 1), "las mismas vistas no avisan");
        let gone = Vec::new();
        session.update(&mut cx, |session, cx| session.set_views(gone, None, cx));
        cx.run_until_parked();
        assert_eq!(events.get(), (2, 2));
        session.read_with(&cx, |session, _| {
            assert_eq!(session.focus().unwrap().label(), "focus: #rust");
        });
    }

    #[test]
    fn los_recientes_se_leen_de_lo_guardado() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state/vaults/x/recientes");
        assert_eq!(read_recents(&file), Recents::default());
        let recents = Recents::parse("b.md\na.md\n");
        super::super::writer::write_recents(&file, &recents.to_text()).unwrap();
        assert_eq!(read_recents(&file), recents);
    }

    #[test]
    fn solo_lo_de_dentro_del_vault_tiene_ruta_del_vault() {
        let root = Path::new("/v");
        let path = |text: &str| relative_to(root, Path::new(text));
        assert_eq!(path("/v/d/a.md"), Some(VaultPath::new("d/a.md").unwrap()));
        assert_eq!(path("/otra/a.md"), None);
        assert_eq!(path("/v"), None);
        assert_eq!(path("/v/.oculta/a.md"), None);
    }

    #[test]
    fn una_nota_sin_guardar_se_ubica_por_lo_que_existe() {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            canonical(&root.join("nueva.md")),
            Some(root.join("nueva.md"))
        );
        assert_eq!(
            canonical(&root.join("no/existe.md")),
            Some(root.join("no/existe.md")),
            "su carpeta se borró desde fuera"
        );
        assert_eq!(canonical(&root.join("no/../existe.md")), None);
    }
}
