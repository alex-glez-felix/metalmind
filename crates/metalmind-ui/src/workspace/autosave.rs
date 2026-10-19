//! El autoguardado (`editor.autosave`): guarda los documentos con archivo y
//! con cambios tras un rato sin editar, al cambiar el documento de un panel,
//! al perder la ventana el foco y antes de cerrarla. Lo que cambió en disco
//! no se pisa: se avisa y lo decide `:w!` o `:e!`. El reloj se inyecta, así
//! que los tests lo mueven a mano.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;
use std::time::{Duration, Instant};

use metalmind_vault::SaveKind;

use super::{DocumentId, Message, Workspace};
use crate::document::{Document, SaveError};

/// Mientras haya un autoguardado en camino, el indicador sigue diciendo
/// "guardado" hasta que los cambios tengan esta cantidad de veces el delay:
/// así no parpadea con cada pausa al escribir.
const QUIET_DELAYS: u32 = 5;

/// Si un autoguardado falla al escribir (un disco lleno, una carpeta sin
/// permisos), se reintenta tras esta cantidad de veces el delay.
const RETRY_DELAYS: u32 = 5;

pub trait Clock: fmt::Debug {
    fn now(&self) -> Instant;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Lo que muestra la barra de estado de un documento con autoguardado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveState {
    Saved,
    Unsaved,
    /// Tiene cambios y su archivo cambió fuera de Metalmind.
    ChangedOnDisk,
}

/// Qué pasó al intentar autoguardar un documento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Autosaved {
    /// No hacía falta: sin archivo, sin cambios o ya en conflicto.
    Skipped,
    Saved,
    /// Su archivo cambió en disco: se avisó y no se tocó.
    ChangedOnDisk,
    /// No se pudo escribir: se avisó.
    Failed,
}

#[derive(Debug)]
pub struct Autosave {
    delay: Duration,
    clock: Rc<dyn Clock>,
    /// `delay` después de la última edición, si desde entonces no se guardó.
    deadline: Option<Instant>,
    /// Desde cuándo tiene cambios cada documento editado.
    unsaved_since: BTreeMap<DocumentId, Instant>,
    /// Los documentos cuyo autoguardado falló desde la última vez que se
    /// guardaron: los reintentos que fallan no vuelven a avisar (el texto del
    /// error cambia con el nombre del temporal).
    failing: BTreeSet<DocumentId>,
}

impl Autosave {
    #[must_use]
    pub fn new(delay: Duration, clock: Rc<dyn Clock>) -> Self {
        Self {
            delay,
            clock,
            deadline: None,
            unsaved_since: BTreeMap::new(),
            failing: BTreeSet::new(),
        }
    }
}

impl Workspace {
    pub fn set_autosave(&mut self, autosave: Option<Autosave>) {
        self.autosave = autosave;
    }

    /// Cuándo hay que llamar a [`Workspace::autosave_if_due`].
    #[must_use]
    pub fn autosave_deadline(&self) -> Option<Instant> {
        self.autosave.as_ref()?.deadline
    }

    /// Guarda lo que tenga cambios si ya pasó el delay desde la última
    /// edición. Devuelve si guardó o avisó algo.
    pub fn autosave_if_due(&mut self) -> bool {
        let due = self
            .autosave
            .as_ref()
            .and_then(|autosave| Some(autosave.deadline? <= autosave.clock.now()))
            .unwrap_or(false);
        due && self.autosave_all()
    }

    /// Guarda ya todo lo que tenga cambios: al perder el foco y antes de
    /// cerrar la ventana. Devuelve si guardó o avisó algo. Si alguna
    /// escritura falló, deja un plazo para reintentar.
    pub fn autosave_all(&mut self) -> bool {
        let Some(autosave) = self.autosave.as_mut() else {
            return false;
        };
        autosave.deadline = None;
        let ids: Vec<DocumentId> = self.documents.iter().map(|(id, _)| id).collect();
        let outcomes: Vec<Autosaved> = ids
            .into_iter()
            .map(|id| self.autosave_document(id))
            .collect();
        if outcomes.contains(&Autosaved::Failed)
            && let Some(autosave) = self.autosave.as_mut()
        {
            autosave.deadline = Some(autosave.clock.now() + autosave.delay * RETRY_DELAYS);
        }
        outcomes
            .iter()
            .any(|&outcome| outcome != Autosaved::Skipped)
    }

    /// Guarda `id` si tiene archivo, cambios y su archivo no cambió en disco.
    pub(super) fn autosave_document(&mut self, id: DocumentId) -> Autosaved {
        if self.autosave.is_none() {
            return Autosaved::Skipped;
        }
        let Some(document) = self.documents.get_mut(id) else {
            return Autosaved::Skipped;
        };
        let wanted = document.path().is_some() && document.is_modified();
        // Un conflicto se vuelve a mirar: el archivo pudo volver a lo que se
        // leyó sin que el watcher avisara.
        let conflict = document.changed_on_disk() && !matches!(document.check_disk(), Ok(false));
        if !wanted || conflict {
            return Autosaved::Skipped;
        }
        match document.save(self.disk.as_ref(), SaveKind::Autosave) {
            Ok(_) => {
                self.saved(id);
                Autosaved::Saved
            }
            Err(SaveError::ChangedOnDisk) => {
                self.message = Some(Message::changed_on_disk(document));
                Autosaved::ChangedOnDisk
            }
            Err(error) => {
                let first = self
                    .autosave
                    .as_mut()
                    .is_some_and(|autosave| autosave.failing.insert(id));
                if first {
                    let name = document.display_name();
                    log::error!("autoguardado de {name}: {error}");
                    let hidden = !self.in_focus(id);
                    self.message = Some(Message::save_failed(hidden.then_some(&*name), &error));
                }
                Autosaved::Failed
            }
        }
    }

    /// Se editó `id`: el autoguardado espera otro delay entero.
    pub(super) fn edited(&mut self, id: DocumentId) {
        let modified = self.documents.get(id).is_some_and(Document::is_modified);
        let Some(autosave) = self.autosave.as_mut() else {
            return;
        };
        let now = autosave.clock.now();
        autosave.deadline = Some(now + autosave.delay);
        if modified {
            autosave.unsaved_since.entry(id).or_insert(now);
        } else {
            autosave.unsaved_since.remove(&id);
        }
    }

    /// `id` quedó igual que su archivo: guardado o releído.
    pub(super) fn saved(&mut self, id: DocumentId) {
        if let Some(autosave) = self.autosave.as_mut() {
            autosave.unsaved_since.remove(&id);
            autosave.failing.remove(&id);
        }
    }

    /// Lo que dice el indicador de `id`; `None` sin autoguardado, cuando la
    /// barra muestra el `[+]` de vim.
    pub(super) fn save_state(&self, id: DocumentId, document: &Document) -> Option<SaveState> {
        let autosave = self.autosave.as_ref()?;
        if !document.is_modified() {
            // Sin archivo escrito (un `[No Name]`, una nota nueva) no hay nada
            // guardado que decir: la barra queda como en vim.
            return document.exists_on_disk().then_some(SaveState::Saved);
        }
        if document.changed_on_disk() {
            return Some(SaveState::ChangedOnDisk);
        }
        // Un reintento en camino no es un guardado: lo último que pasó es que falló.
        if autosave.failing.contains(&id) {
            return Some(SaveState::Unsaved);
        }
        let quiet = autosave.delay * QUIET_DELAYS;
        let coming = autosave.deadline.is_some() && document.path().is_some();
        let fresh = autosave
            .unsaved_since
            .get(&id)
            .is_some_and(|&since| autosave.clock.now() < since + quiet);
        Some(if coming && fresh {
            SaveState::Saved
        } else {
            SaveState::Unsaved
        })
    }
}

#[cfg(test)]
#[path = "autosave_tests.rs"]
mod tests;
