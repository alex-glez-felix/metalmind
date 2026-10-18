//! El vault del finder mientras la ventana lo usa: el índice que mantiene el
//! watcher y las notas recientes, guardadas en el directorio de estado del
//! vault. Abrirlo y guardar pasan en el executor de fondo; los cambios del
//! watcher llegan al hilo de la UI por un canal.

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::{fs, thread};

use gpui_kit::{App, AppContext as _, Context, Entity, Task};
use nota_config::Paths;
use nota_vault::{Vault, VaultError, VaultPath, VaultUpdate};

use super::{Note, Recents, notes};

/// El archivo de los recientes dentro del estado del vault.
const RECENTS_FILE: &str = "recientes";

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("no se pudo arrancar el hilo que sigue al vault: {0}")]
    Thread(io::Error),
}

pub struct VaultSession {
    vault: Vault,
    recents: Recents,
    /// `None` si no hay directorio de estado: se recuerdan solo en memoria.
    recents_file: Option<PathBuf>,
    notes: Arc<[Note]>,
    _updates: Task<()>,
    /// La última escritura de los recientes; cada una espera a la anterior.
    saving: Option<Task<()>>,
}

/// Lo que se lee del disco al abrir, fuera del hilo de la UI.
struct Opened {
    vault: Vault,
    updates: mpsc::Receiver<VaultUpdate>,
    recents: Recents,
    recents_file: Option<PathBuf>,
}

impl Opened {
    fn load(root: &Path, paths: Option<&Paths>) -> Result<Self, VaultError> {
        let (vault, updates) = Vault::open(root)?;
        let recents_file = paths.map(|paths| paths.vault_state(vault.root()).join(RECENTS_FILE));
        let recents = recents_file
            .as_deref()
            .map(read_recents)
            .unwrap_or_default();
        Ok(Self {
            vault,
            updates,
            recents,
            recents_file,
        })
    }
}

impl VaultSession {
    /// Abre `root` como vault, con los recientes guardados en `paths`.
    pub fn open(
        root: PathBuf,
        paths: Option<Paths>,
        cx: &App,
    ) -> Task<Result<Entity<Self>, SessionError>> {
        let opening = cx.background_spawn(async move { Opened::load(&root, paths.as_ref()) });
        cx.spawn(async move |cx| {
            let opened = opening.await?;
            let (sender, receiver) = async_channel::unbounded();
            forward(opened.updates, sender).map_err(SessionError::Thread)?;
            Ok(cx.new(|cx| {
                let updates = cx.spawn(async move |this, cx| {
                    while let Ok(update) = receiver.recv().await {
                        let applied =
                            this.update(cx, |this: &mut Self, cx| this.apply(&update, cx));
                        if applied.is_err() {
                            return;
                        }
                    }
                });
                let notes = notes(&opened.vault.snapshot(), &opened.recents);
                Self {
                    vault: opened.vault,
                    recents: opened.recents,
                    recents_file: opened.recents_file,
                    notes,
                    _updates: updates,
                    saving: None,
                }
            }))
        })
    }

    /// La raíz, sin enlaces simbólicos.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.vault.root()
    }

    /// Lo que lista el finder ahora.
    #[must_use]
    pub fn notes(&self) -> Arc<[Note]> {
        Arc::clone(&self.notes)
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

    fn apply(&mut self, update: &VaultUpdate, cx: &mut Context<Self>) {
        let mut moved = false;
        for change in &update.changes {
            moved |= self.recents.apply(change);
        }
        if moved {
            self.save(cx);
        }
        self.notes = notes(&update.snapshot, &self.recents);
        cx.notify();
    }

    fn recents_changed(&mut self, cx: &mut Context<Self>) {
        self.notes = notes(&self.vault.snapshot(), &self.recents);
        self.save(cx);
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.recents_file.clone() else {
            return;
        };
        let text = self.recents.to_text();
        let previous = self.saving.take();
        self.saving = Some(cx.background_spawn(async move {
            if let Some(previous) = previous {
                previous.await;
            }
            if let Err(error) = write_recents(&file, &text) {
                log::warn!("no se guardaron las notas recientes: {error}");
            }
        }));
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
        .name("nota-ui vault".to_owned())
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
fn relative_to(root: &Path, path: &Path) -> Option<VaultPath> {
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

/// La ruta sin enlaces simbólicos; si el archivo todavía no existe (una
/// nota nueva sin guardar), la de su carpeta.
fn canonical(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path).ok().or_else(|| {
        let name = path.file_name()?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let folder = fs::canonicalize(parent.unwrap_or(Path::new("."))).ok()?;
        Some(folder.join(name))
    })
}

fn read_recents(file: &Path) -> Recents {
    match nota_vault::read(file) {
        Ok(text) => Recents::parse(&text.unwrap_or_default()),
        Err(error) => {
            log::warn!("no se leyeron las notas recientes: {error}");
            Recents::default()
        }
    }
}

fn write_recents(file: &Path, text: &str) -> Result<(), nota_vault::FileError> {
    if let Some(folder) = file.parent()
        && let Err(source) = fs::create_dir_all(folder)
    {
        return Err(nota_vault::FileError::Write {
            path: folder.to_owned(),
            source,
        });
    }
    nota_vault::write_atomic(file, |out| out.write_all(text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_recientes_se_guardan_y_se_leen_creando_la_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state/vaults/x/recientes");
        assert_eq!(read_recents(&file), Recents::default());
        let recents = Recents::parse("b.md\na.md\n");
        write_recents(&file, &recents.to_text()).unwrap();
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
    fn una_nota_sin_guardar_se_ubica_por_su_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            canonical(&root.join("nueva.md")),
            Some(root.join("nueva.md"))
        );
        assert_eq!(canonical(&root.join("no/existe.md")), None);
    }
}
