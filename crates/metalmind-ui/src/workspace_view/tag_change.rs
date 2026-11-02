//! `:tagrename` y `:tagmerge` en la ventana: arma el plan con los tags del
//! vault y el texto de los buffers sin guardar, muestra su vista previa,
//! aplica lo que quedó (al disco con [`Vault::apply`], y a cada buffer con
//! cambios como un paso de su undo) y lo guarda para `:vaultundo` (05 §5).
//! Desde la vista previa se abre la línea de un cambio y se vuelve a ella
//! con `space r`.

use std::collections::HashSet;

use gpui_kit::{AppContext as _, Context, Entity, Window};
use metalmind_config::VaultConfig;
use metalmind_keymap::WindowCommand;
use metalmind_vault::{
    Applied, ApplyError, Changes, EntryKind, FileEdits, FileOpError, Rename, Vault, VaultPath,
};

use super::WorkspaceView;
use super::buffer_edits::{Routed, route};
use super::overlay::Overlay;
use super::vault_undo::ListFollowed;
use crate::change_preview::{ChangePreviewEvent, ChangePreviewView};
use crate::picker::Open;
use crate::tag_change::{TagRequest, config_path};
use crate::workspace::{LinePosition, Message, MessageId, ToastAction, shown_path};

/// El botón del aviso tras aplicar un cambio al vault.
const UNDO: ToastAction = ToastAction {
    label: "Deshacer",
    command: WindowCommand::UndoVaultChange,
};

/// El botón del aviso tras dejar una vista previa para abrir una línea.
const RESUME: ToastAction = ToastAction {
    label: "Volver",
    command: WindowCommand::ResumeTagChange,
};

/// Una vista previa de tags que se dejó para abrir la línea de un cambio:
/// lo que se pidió y cómo estaba, para rehacerla al volver con lo que se
/// haya editado entre medio.
pub(super) struct ParkedTagChange {
    request: TagRequest,
    view: Entity<ChangePreviewView>,
    toast: MessageId,
}

impl WorkspaceView {
    pub(super) fn change_tags(
        &mut self,
        request: &TagRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(parked) = self.parked_tag_change.take() {
            self.retire_toast(parked.toast, cx);
        }
        self.show_tag_change(request, None, window, cx);
    }

    /// `space r`: la vista previa que se dejó, rehecha sobre lo de ahora y
    /// con las filas quitadas y la elegida como estaban.
    pub(super) fn resume_tag_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Una macro pudo cambiar los paneles antes.
        self.sync_editors(cx);
        let Some(parked) = self.parked_tag_change.take() else {
            let text = "no hay vista previa a la que volver: space r vuelve a la de un cambio de \
                        tags tras abrir una de sus líneas";
            return self.tell(Message::Info(text.to_owned()), cx);
        };
        self.retire_toast(parked.toast, cx);
        self.show_tag_change(&parked.request, Some(&parked.view), window, cx);
    }

    /// La vista previa de `request` sobre los tags del vault, con los
    /// buffers con cambios sin guardar como si estuvieran guardados: sus
    /// ediciones van a ellos. Con `old`, se queda con lo que se había
    /// quitado y elegido en ella.
    fn show_tag_change(
        &mut self,
        request: &TagRequest,
        old: Option<&Entity<ChangePreviewView>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let texts = self.unsaved_texts(cx);
        let unsaved = self.unsaved_paths(cx);
        let session = self.session.read(cx);
        let notes = texts
            .notes
            .iter()
            .filter(|(path, _)| EntryKind::of_file(path) == EntryKind::Note)
            .map(|(path, text)| (path, text.as_str()));
        let tags = session.tags().with_unsaved(notes);
        let config = texts.config(session.root());
        let mut preview = match request.preview(&tags, &config, |path| unsaved.contains(path)) {
            Ok(Some(preview)) => preview,
            Ok(None) => {
                let text = format!("{}: no hay nada que cambiar", request.title());
                return self.tell(Message::Info(text), cx);
            }
            Err(error) => return self.tell(Message::Error(error.to_string()), cx),
        };
        if let Some(old) = old {
            preview.carry_over(old.read(cx).preview());
        }
        let view = cx.new(|cx| ChangePreviewView::new(preview, cx).with_open_line());
        let asked = request.clone();
        let events = cx.subscribe_in(&view, window, move |this, view, event, window, cx| {
            this.on_tag_change_event(view, *event, &asked, window, cx);
        });
        self.show(
            Overlay::ChangePreview {
                view,
                rename: None,
                _events: events,
            },
            window,
            cx,
        );
    }

    fn on_tag_change_event(
        &mut self,
        view: &Entity<ChangePreviewView>,
        event: ChangePreviewEvent,
        request: &TagRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event == ChangePreviewEvent::OpenLine {
            return self.park_tag_change(view, request, window, cx);
        }
        let apply = (event == ChangePreviewEvent::Apply).then(|| {
            let preview = view.read(cx).preview();
            (
                preview.title().to_owned(),
                preview.summary(),
                preview.changes(),
            )
        });
        self.close_overlay(window, cx);
        let Some((title, summary, changes)) = apply else {
            return;
        };
        // La lista de notas sigue a un tag renombrado o fusionado.
        let renamed = match request {
            TagRequest::Rename { from, to } => Some((from, to)),
            TagRequest::Merge { from, into } => Some((from, into)),
            TagRequest::Page { .. } => None,
        };
        if self.apply_tag_changes(title, &summary, &changes, cx)
            && let Some((from, to)) = renamed
            && self
                .list
                .update(cx, |list, cx| list.rename_tag(from, to, cx))
        {
            let choices = self.list.read(cx).source_choices();
            self.vault_undo.list_followed(ListFollowed {
                from: from.clone(),
                to: to.clone(),
                choices,
            });
        }
    }

    /// Deja la vista previa para abrir el archivo del cambio elegido en su
    /// línea; el aviso dice cómo volver.
    fn park_tag_change(
        &mut self,
        view: &Entity<ChangePreviewView>,
        request: &TagRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preview = view.read(cx).preview();
        let Some((path, line)) = preview
            .row(preview.selected())
            .and_then(|row| Some((row.path.clone(), row.line?)))
        else {
            return;
        };
        self.close_overlay(window, cx);
        let at = LinePosition {
            line: line.saturating_sub(1),
            column: 0,
        };
        self.open_note_at(&path, at, Open::Here, window, cx);
        let toast = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            let text = "la vista previa sigue guardada".to_owned();
            workspace.show_message_with(Message::Info(text), RESUME)
        });
        self.parked_tag_change = Some(ParkedTagChange {
            request: request.clone(),
            view: view.clone(),
            toast,
        });
    }

    /// Aplica `changes` y lo guarda para `:vaultundo`; `summary` es el
    /// aviso si todo sale bien. Las ediciones de los archivos con cambios
    /// sin guardar van a su buffer, como un paso de su undo, solo si lo
    /// demás se escribió entero. Devuelve si se aplicó entero.
    pub(super) fn apply_tag_changes(
        &mut self,
        title: String,
        summary: &str,
        changes: &Changes,
        cx: &mut Context<Self>,
    ) -> bool {
        let unsaved = self.unsaved_paths(cx);
        let Routed { disk, buffers } = route(changes, |path| unsaved.contains(path));
        if let Err(error) = self.check_buffers(&buffers, cx) {
            self.tell(
                Message::Error(format!("{title}: no se cambió nada: {error}")),
                cx,
            );
            return false;
        }
        let vault = self.session.read(cx).vault();
        let message = match vault.apply(&disk) {
            Ok(applied) => {
                self.follow_applied(&vault, &applied, cx);
                let (undo, failed) = self.edit_buffers(&buffers, cx);
                self.remember_change(title.clone(), applied, undo, cx);
                if failed.is_empty() {
                    self.tell_undoable(summary.to_owned(), cx);
                    return true;
                }
                let failed: Vec<String> = failed
                    .into_iter()
                    .map(|(path, error)| format!("{path} ({error})"))
                    .collect();
                Message::Error(format!(
                    "{title}: no se cambió {}. :vaultundo deshace lo demás",
                    failed.join(", ")
                ))
            }
            Err(ApplyError::Interrupted { applied, cause }) if applied.is_empty() => {
                Message::Error(format!("{title}: no se cambió nada: {cause}"))
            }
            Err(ApplyError::Interrupted { applied, cause }) => {
                self.follow_applied(&vault, &applied, cx);
                let mut text =
                    format!("{title} se paró a medias ({cause}). :vaultundo deshace lo hecho");
                if !buffers.is_empty() {
                    let paths: Vec<&str> = buffers.iter().map(|file| file.path.as_str()).collect();
                    text = format!(
                        "{text}; los buffers sin guardar no se tocaron: {}",
                        paths.join(", ")
                    );
                }
                let message = Message::Error(text);
                self.remember_change(title, *applied, Vec::new(), cx);
                message
            }
            Err(error) => Message::Error(format!("{title}: no se cambió nada: {error}")),
        };
        self.tell(message, cx);
        false
    }

    /// Los documentos abiertos y la lista siguen a lo que `applied` escribió
    /// y renombró.
    fn follow_applied(&mut self, vault: &Vault, applied: &Applied, cx: &mut Context<Self>) {
        self.follow_renames(vault, applied.renamed(), cx);
        self.reread(vault, applied.touched(), cx);
        self.follow_saved_views(applied, cx);
    }

    /// `applied`, y `buffers`, lo que devuelve los buffers que recibieron
    /// el cambio, pasan a ser lo que deshace `:vaultundo`. El "Deshacer"
    /// del aviso de un cambio anterior ya no vale.
    pub(super) fn remember_change(
        &mut self,
        title: String,
        applied: Applied,
        buffers: Vec<FileEdits>,
        cx: &mut Context<Self>,
    ) {
        if let Some(toast) = self.vault_undo.take_toast() {
            self.retire_toast(toast, cx);
        }
        self.vault_undo.remember(title, applied, buffers);
    }

    /// El aviso de lo que se acaba de aplicar, con "Deshacer".
    pub(super) fn tell_undoable(&mut self, text: String, cx: &mut Context<Self>) {
        let toast = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.show_message_with(Message::Info(text), UNDO)
        });
        self.vault_undo.set_toast(toast);
    }

    pub(super) fn retire_toast(&mut self, toast: MessageId, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.retire_message(toast);
            cx.notify();
        });
    }

    /// `:tagpage`: la página de `tag`, o la de su tag si es un alias. Si no
    /// existe se crea vacía, con su carpeta, como "Agregar descripción"
    /// (05 §7).
    pub(super) fn open_tag_page(&mut self, tag: &str, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        let (vault, root) = (session.vault(), session.root().to_owned());
        let created = session
            .tags()
            .page_path(tag)
            .map_err(|error| error.to_string())
            .and_then(|page| match vault.create_note(&page, "") {
                Ok(()) | Err(FileOpError::AlreadyExists(_)) => Ok(page),
                Err(error) => Err(error.to_string()),
            });
        match created {
            Ok(page) => self.edit_note(&shown_path(&root.join(page.as_str())), window, cx),
            Err(error) => self.tell(Message::Error(error), cx),
        }
    }

    /// Con la vista previa abierta, lo que se guardó desde que se abrió deja
    /// de bloquear, como con el autoguardado.
    pub(super) fn refresh_change_preview(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::ChangePreview { view, .. }) = &self.overlay else {
            return;
        };
        let view = view.clone();
        let unsaved = self.unsaved_paths(cx);
        view.update(cx, |view, cx| {
            view.preview_mut()
                .set_unsaved(|path| unsaved.contains(path));
            cx.notify();
        });
    }

    /// Los archivos del vault con cambios sin guardar en un buffer.
    pub(super) fn unsaved_paths(&self, cx: &Context<Self>) -> HashSet<VaultPath> {
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
    pub(super) fn follow_renames<'r>(
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
    pub(super) fn reread(
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

    /// Si `applied` reescribió la config del vault, la sesión relee sus
    /// vistas: la lista, el panel y el focus traen las notas de su consulta
    /// nueva.
    pub(super) fn follow_saved_views(&mut self, applied: &Applied, cx: &mut Context<Self>) {
        if applied
            .touched()
            .any(|path| path.as_str() == VaultConfig::PATH)
        {
            self.vault_views(cx);
        }
    }

    pub(super) fn tell(&mut self, message: Message, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.show_message(message);
            cx.notify();
        });
    }
}
