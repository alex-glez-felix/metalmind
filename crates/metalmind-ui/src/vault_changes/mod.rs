//! Cambiar el vault: renombrar una nota con sus links (03 §4), renombrar o
//! fusionar tags y etiquetar una nota (05 §5), con su vista previa, y deshacer
//! lo último que se aplicó. Es dueño de ese flujo: la ventana pide, y de lo que
//! sigue solo recibe [`VaultChangesEvent`].

mod buffers;
mod rename;
mod rename_plan;
mod tags;
mod undo;

use std::collections::HashSet;

use gpui_kit::{Context, Entity, EventEmitter, Subscription, Task, WeakEntity};
use metalmind_config::VaultConfig;
use metalmind_vault::{Applied, Changes, Rename, Vault, VaultPath};

use self::rename::PlannedRename;
use self::rename_plan::RenamePreview;
use self::tags::ParkedTagChange;
use self::undo::VaultUndo;
use crate::change_preview::{ChangePreview, ChangePreviewEvent, ChangePreviewView};
use crate::finder::{VaultChanged, VaultSession};
use crate::note_list::NoteListView;
use crate::tag_change::{TagRequest, config_path};
use crate::workspace::{LinePosition, Message, Workspace};

/// Lo que la ventana hace por [`VaultChanges`].
pub enum VaultChangesEvent {
    ShowPreview(Entity<ChangePreviewView>),
    ClosePreview {
        focus_sidebar: bool,
    },
    OpenLine {
        path: VaultPath,
        at: LinePosition,
    },
    /// Un renombre pedido ya tiene su plan: se sigue con
    /// [`VaultChanges::finish_rename`], que necesita saber si hay algo abierto.
    RenamePlanned,
    Renamed(VaultPath),
    /// Se reescribió la config del vault, y con ella sus vistas guardadas.
    SavedViewsChanged,
}

pub struct VaultChanges {
    workspace: Entity<Workspace>,
    session: Entity<VaultSession>,
    list: Entity<NoteListView>,
    vault_undo: VaultUndo,
    preview: Option<OpenPreview>,
    /// La vista previa de tags que se dejó para abrir una línea.
    parked_tag_change: Option<ParkedTagChange>,
    /// El plan de un renombre que se está armando, antes de su vista previa.
    renaming: Option<Task<()>>,
    planned_rename: Option<PlannedRename>,
    _subscriptions: [Subscription; 2],
}

/// La vista previa que se pidió mostrar. Es de la ventana: si la cambia por
/// otra cosa, `view` deja de valer y esto ya no hace nada.
struct OpenPreview {
    view: WeakEntity<ChangePreviewView>,
    flow: PreviewFlow,
    _events: Subscription,
}

enum PreviewFlow {
    Rename(RenamePreview),
    Tags(TagRequest),
}

/// Lo que quedó en una vista previa al aplicarla.
struct Accepted {
    title: String,
    summary: String,
    changes: Changes,
}

impl Accepted {
    fn of(preview: &ChangePreview) -> Self {
        Self {
            title: preview.title().to_owned(),
            summary: preview.summary(),
            changes: preview.changes(),
        }
    }
}

impl EventEmitter<VaultChangesEvent> for VaultChanges {}

impl VaultChanges {
    pub fn new(
        workspace: Entity<Workspace>,
        session: Entity<VaultSession>,
        list: Entity<NoteListView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let edits = cx.observe(&workspace, |changes, _, cx| changes.refresh_unsaved(cx));
        let batches = cx.subscribe(&session, |changes, _, VaultChanged(update), cx| {
            changes.refresh_rename_preview(&update.links, cx);
        });
        Self {
            workspace,
            session,
            list,
            vault_undo: VaultUndo::default(),
            preview: None,
            parked_tag_change: None,
            renaming: None,
            planned_rename: None,
            _subscriptions: [edits, batches],
        }
    }

    /// Muestra `view`, que sigue a `flow` hasta cerrarse.
    fn open_preview(
        &mut self,
        view: Entity<ChangePreviewView>,
        flow: PreviewFlow,
        cx: &mut Context<Self>,
    ) {
        let events = cx.subscribe(&view, |changes, view, event, cx| {
            changes.on_preview_event(&view, *event, cx);
        });
        self.preview = Some(OpenPreview {
            view: view.downgrade(),
            flow,
            _events: events,
        });
        cx.emit(VaultChangesEvent::ShowPreview(view));
    }

    fn on_preview_event(
        &mut self,
        view: &Entity<ChangePreviewView>,
        event: ChangePreviewEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = &self.preview else {
            return;
        };
        match &open.flow {
            PreviewFlow::Rename(rename)
                if event == ChangePreviewEvent::Apply && !rename.is_current() =>
            {
                let text = "la vista previa se está rehaciendo con los cambios de ahora; aplica \
                            cuando termine";
                self.tell(Message::Info(text.to_owned()), cx);
            }
            PreviewFlow::Tags(request) if event == ChangePreviewEvent::OpenLine => {
                let request = request.clone();
                self.park_tag_change(view, request, cx);
            }
            _ => self.close_preview(view, event, cx),
        }
    }

    fn close_preview(
        &mut self,
        view: &Entity<ChangePreviewView>,
        event: ChangePreviewEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = self.preview.take() else {
            return;
        };
        cx.emit(VaultChangesEvent::ClosePreview {
            focus_sidebar: matches!(open.flow, PreviewFlow::Rename(_)),
        });
        if event != ChangePreviewEvent::Apply {
            return;
        }
        let accepted = Accepted::of(view.read(cx).preview());
        match open.flow {
            PreviewFlow::Rename(rename) => self.apply_rename(&rename, accepted, cx),
            PreviewFlow::Tags(request) => self.apply_tag_request(&request, accepted, cx),
        }
    }

    /// Con la vista previa abierta, lo que se guardó desde que se abrió deja
    /// de bloquear, como con el autoguardado.
    fn refresh_unsaved(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.preview.as_ref().and_then(|open| open.view.upgrade()) else {
            return;
        };
        let unsaved = self.unsaved_paths(cx);
        view.update(cx, |view, cx| {
            view.preview_mut()
                .set_unsaved(|path| unsaved.contains(path));
            cx.notify();
        });
    }

    /// Los archivos del vault con cambios sin guardar en un buffer.
    fn unsaved_paths(&self, cx: &Context<Self>) -> HashSet<VaultPath> {
        let session = self.session.read(cx);
        let root = session.root();
        self.workspace
            .read(cx)
            .unsaved_files()
            .filter_map(|(file, _)| session.vault_path(file).or_else(|| config_path(root, file)))
            .collect()
    }

    /// Los documentos abiertos de lo que renombró el vault pasan a su ruta
    /// nueva.
    fn follow_renames<'r>(
        &self,
        vault: &Vault,
        renames: impl Iterator<Item = &'r Rename>,
        cx: &mut Context<Self>,
    ) {
        let root = vault.root();
        self.workspace.update(cx, |workspace, cx| {
            for rename in renames {
                let (from, to) = (rename.from.as_str(), rename.to.as_str());
                workspace.moved(&root.join(from), &root.join(to));
            }
            cx.notify();
        });
    }

    /// Los documentos abiertos sin cambios de lo que se escribió se releen
    /// ya, sin esperar al watcher: lo que se teclee enseguida va sobre el
    /// texto nuevo.
    fn reread(
        &self,
        vault: &Vault,
        paths: impl Iterator<Item = VaultPath>,
        cx: &mut Context<Self>,
    ) {
        let root = vault.root();
        self.workspace.update(cx, |workspace, cx| {
            for path in paths {
                workspace.file_changed(&root.join(path.as_str()));
            }
            cx.notify();
        });
    }

    /// Si `applied` reescribió la config del vault, la ventana relee sus
    /// vistas: la lista, el panel y el focus traen las notas de su consulta
    /// nueva.
    fn follow_saved_views(applied: &Applied, cx: &mut Context<Self>) {
        if applied
            .touched()
            .any(|path| path.as_str() == VaultConfig::PATH)
        {
            cx.emit(VaultChangesEvent::SavedViewsChanged);
        }
    }

    fn tell(&mut self, message: Message, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.show_message(message);
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests;
