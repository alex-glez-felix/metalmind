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
use metalmind_config::Paths;
use metalmind_vault::tags::Tags;
use metalmind_vault::{Unwatched, Vault, VaultPath, VaultUpdate};

use super::writer::RecentsWriter;
use super::{Note, Recents, notes};

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

pub struct VaultSession {
    /// Compartido con el panel lateral, que hace las operaciones de archivo.
    vault: Rc<Vault>,
    recents: Recents,
    /// `None` si no hay directorio de estado: se recuerdan solo en memoria.
    writer: Option<RecentsWriter>,
    notes: Arc<[Note]>,
    tags: Arc<Tags>,
    unwatched: Option<Unwatched>,
    _updates: Task<()>,
    _quit: Subscription,
}

impl EventEmitter<VaultChanged> for VaultSession {}
impl EventEmitter<WatchLost> for VaultSession {}

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
        Self {
            vault: Rc::new(vault),
            recents,
            writer,
            notes,
            tags,
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

    /// Lo que lista el finder ahora.
    #[must_use]
    pub fn notes(&self) -> Arc<[Note]> {
        Arc::clone(&self.notes)
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
        self.notes = notes(&update.snapshot, &self.recents);
        self.tags = Arc::clone(&update.tags);
        cx.emit(VaultChanged(update));
        // Se pierde al aparecer carpetas, así que llega con un lote.
        if let (None, Some(reason)) = (self.unwatched, self.vault.unwatched()) {
            self.unwatched = Some(reason);
            cx.emit(WatchLost(reason));
        }
        cx.notify();
    }

    fn recents_changed(&mut self, cx: &mut Context<Self>) {
        self.notes = notes(&self.vault.snapshot(), &self.recents);
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
    use super::*;

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
