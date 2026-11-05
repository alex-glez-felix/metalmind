//! `:tagrename` y `:tagmerge`: arma el plan con los tags del vault y el texto
//! de los buffers sin guardar, muestra su vista previa, aplica lo que quedó (al
//! disco con [`Vault::apply`], y a cada buffer con cambios como un paso de su
//! undo) y lo guarda para `:vaultundo` (05 §5). Desde la vista previa se abre
//! la línea de un cambio y se vuelve a ella con `space r`. Etiquetar una nota
//! desde la lista se aplica igual, sin vista previa.

use gpui_kit::{AppContext as _, Context, Entity};
use metalmind_keymap::WindowCommand;
use metalmind_vault::{Applied, ApplyError, Changes, EntryKind, Vault, VaultPath};

use super::buffers::{Routed, route};
use super::undo::ListFollowed;
use super::{Accepted, PreviewFlow, VaultChanges, VaultChangesEvent};
use crate::change_preview::ChangePreviewView;
use crate::note_list::tag_edit;
use crate::tag_change::TagRequest;
use crate::workspace::{LinePosition, Message, MessageId, ToastAction};

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

impl VaultChanges {
    pub fn change_tags(&mut self, request: &TagRequest, cx: &mut Context<Self>) {
        if let Some(parked) = self.parked_tag_change.take() {
            self.retire_toast(parked.toast, cx);
        }
        self.show_tag_change(request, None, cx);
    }

    /// `space r`: la vista previa que se dejó, rehecha sobre lo de ahora y
    /// con las filas quitadas y la elegida como estaban.
    pub fn resume_tag_change(&mut self, cx: &mut Context<Self>) {
        let Some(parked) = self.parked_tag_change.take() else {
            let text = "no hay vista previa a la que volver: space r vuelve a la de un cambio de \
                        tags tras abrir una de sus líneas";
            return self.tell(Message::Info(text.to_owned()), cx);
        };
        self.retire_toast(parked.toast, cx);
        self.show_tag_change(&parked.request, Some(&parked.view), cx);
    }

    /// La vista previa de `request` sobre los tags del vault, con los
    /// buffers con cambios sin guardar como si estuvieran guardados: sus
    /// ediciones van a ellos. Con `old`, se queda con lo que se había
    /// quitado y elegido en ella.
    fn show_tag_change(
        &mut self,
        request: &TagRequest,
        old: Option<&Entity<ChangePreviewView>>,
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
        self.open_preview(view, PreviewFlow::Tags(request.clone()), cx);
    }

    /// Aplica lo que quedó en la vista previa de `request`. La lista de notas
    /// sigue a un tag renombrado o fusionado.
    pub(super) fn apply_tag_request(
        &mut self,
        request: &TagRequest,
        accepted: Accepted,
        cx: &mut Context<Self>,
    ) {
        let Accepted {
            title,
            summary,
            changes,
        } = accepted;
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
    pub(super) fn park_tag_change(
        &mut self,
        view: &Entity<ChangePreviewView>,
        request: TagRequest,
        cx: &mut Context<Self>,
    ) {
        let preview = view.read(cx).preview();
        let Some((path, line)) = preview
            .row(preview.selected())
            .and_then(|row| Some((row.path.clone(), row.line?)))
        else {
            return;
        };
        self.preview = None;
        cx.emit(VaultChangesEvent::ClosePreview {
            focus_sidebar: false,
        });
        let at = LinePosition {
            line: line.saturating_sub(1),
            column: 0,
        };
        cx.emit(VaultChangesEvent::OpenLine { path, at });
        let toast = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            let text = "la vista previa sigue guardada".to_owned();
            workspace.show_message_with(Message::Info(text), RESUME)
        });
        self.parked_tag_change = Some(ParkedTagChange {
            request,
            view: view.clone(),
            toast,
        });
    }

    /// Agrega `tags` al frontmatter de `note`. Como un cambio de tags, si
    /// tiene cambios sin guardar va a su buffer, y `:vaultundo` lo deshace.
    pub fn tag_note(&mut self, note: &VaultPath, tags: &[String], cx: &mut Context<Self>) {
        let root = self.session.read(cx).root().to_owned();
        let file = root.join(note.as_str());
        let buffer = self
            .unsaved_paths(cx)
            .contains(note)
            .then(|| self.workspace.read(cx).file_text(&file))
            .flatten();
        let text = match buffer.map_or_else(|| metalmind_vault::read(&file), |text| Ok(Some(text)))
        {
            Ok(Some(text)) => text,
            Ok(None) => String::new(),
            Err(error) => return self.tell(Message::Error(error.to_string()), cx),
        };
        let edits = match tag_edit(note, &text, tags) {
            Ok(Some(edits)) => edits,
            Ok(None) => {
                let text = format!("{note} ya tiene {}", tags.join(" "));
                return self.tell(Message::Info(text), cx);
            }
            Err(error) => {
                let text = format!("no se etiquetó {note}: {error}");
                return self.tell(Message::Error(text), cx);
            }
        };
        let changes = Changes {
            edits: vec![edits],
            ..Changes::default()
        };
        let summary = format!("{note}: {}", tags.join(" "));
        self.apply_tag_changes(format!("etiquetar {note}"), &summary, &changes, cx);
    }

    /// Aplica `changes` y lo guarda para `:vaultundo`; `summary` es el
    /// aviso si todo sale bien. Las ediciones de los archivos con cambios
    /// sin guardar van a su buffer, como un paso de su undo, solo si lo
    /// demás se escribió entero. Devuelve si se aplicó entero.
    fn apply_tag_changes(
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
        Self::follow_saved_views(applied, cx);
    }
}
