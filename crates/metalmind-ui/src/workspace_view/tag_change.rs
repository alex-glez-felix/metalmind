//! `:tagrename`, `:tagmerge` y `:tagundo` en la ventana: arma el plan con
//! los tags del vault, muestra su vista previa, aplica lo que quedó con
//! [`Vault::apply`] y guarda lo aplicado para deshacerlo (05 §5).

use std::collections::{BTreeSet, HashSet};

use gpui_kit::{AppContext as _, Context, Entity, Window};
use metalmind_vault::{
    Applied, ApplyError, Changes, FileOpError, Rename, UndoReport, Vault, VaultPath,
};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::change_preview::{ChangePreviewEvent, ChangePreviewView};
use crate::tag_change::TagRequest;
use crate::workspace::{Message, shown_path};

/// El último cambio de tags aplicado, para `:tagundo`.
pub(super) struct LastTagChange {
    title: String,
    applied: Applied,
    /// El tag del que la lista de notas pasó a otro, a cuál y cuántas
    /// fuentes había elegido el usuario: deshacer el cambio la devuelve si
    /// no eligió otra después.
    list_followed: Option<(String, String, u64)>,
}

impl WorkspaceView {
    pub(super) fn change_tags(
        &mut self,
        request: &TagRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let unsaved = self.unsaved_paths(cx);
        let tags = self.session.read(cx).tags();
        let preview = match request.preview(&tags, |path| unsaved.contains(path)) {
            Ok(Some(preview)) => preview,
            Ok(None) => {
                let text = format!("{}: no hay nada que cambiar", request.title());
                return self.tell(Message::Info(text), cx);
            }
            Err(error) => return self.tell(Message::Error(error.to_string()), cx),
        };
        let view = cx.new(|cx| ChangePreviewView::new(preview, cx));
        // La lista de notas sigue a un tag renombrado o fusionado.
        let renamed = match request {
            TagRequest::Rename { from, to } => Some((from.clone(), to.clone())),
            TagRequest::Merge { from, into } => Some((from.clone(), into.clone())),
            TagRequest::Page { .. } => None,
        };
        let events = cx.subscribe_in(&view, window, move |this, view, event, window, cx| {
            this.on_tag_change_event(view, *event, renamed.as_ref(), window, cx);
        });
        self.show(
            Overlay::ChangePreview {
                view,
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
        renamed: Option<&(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        if self.apply_tag_changes(title, &summary, &changes, cx)
            && let Some((from, to)) = renamed
            && self
                .list
                .update(cx, |list, cx| list.rename_tag(from, to, cx))
            && let Some(last) = &mut self.last_tag_change
        {
            let choices = self.list.read(cx).source_choices();
            last.list_followed = Some((from.clone(), to.clone(), choices));
        }
    }

    /// Aplica `changes` y lo guarda para `:tagundo`; `summary` es el aviso si
    /// todo sale bien. Devuelve si se aplicó entero.
    pub(super) fn apply_tag_changes(
        &mut self,
        title: String,
        summary: &str,
        changes: &Changes,
        cx: &mut Context<Self>,
    ) -> bool {
        let vault = self.session.read(cx).vault();
        let mut whole = false;
        let message = match vault.apply(changes) {
            Ok(applied) => {
                whole = true;
                self.follow_renames(&vault, applied.renamed(), cx);
                self.reread(&vault, applied.touched(), cx);
                self.last_tag_change = Some(LastTagChange {
                    title,
                    applied,
                    list_followed: None,
                });
                // Corto: la barra recorta lo que no cabe, y el título ya se vio.
                Message::Info(format!("{summary}; :tagundo lo deshace"))
            }
            Err(ApplyError::Interrupted { applied, cause }) if applied.is_empty() => {
                Message::Error(format!("{title}: no se cambió nada: {cause}"))
            }
            Err(ApplyError::Interrupted { applied, cause }) => {
                self.follow_renames(&vault, applied.renamed(), cx);
                self.reread(&vault, applied.touched(), cx);
                let message = Message::Error(format!(
                    "{title} se paró a medias ({cause}). :tagundo deshace lo hecho"
                ));
                self.last_tag_change = Some(LastTagChange {
                    title,
                    applied: *applied,
                    list_followed: None,
                });
                message
            }
            Err(error) => Message::Error(format!("{title}: no se cambió nada: {error}")),
        };
        self.tell(message, cx);
        whole
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

    pub(super) fn undo_tag_change(&mut self, cx: &mut Context<Self>) {
        let Some(last) = self.last_tag_change.take() else {
            let text = "no hay ningún cambio de tags que deshacer".to_owned();
            return self.tell(Message::Error(text), cx);
        };
        let unsaved = self.unsaved_paths(cx);
        let busy: BTreeSet<VaultPath> = last
            .applied
            .touched()
            .filter(|path| unsaved.contains(path))
            .collect();
        if !busy.is_empty() {
            let paths: Vec<&str> = busy.iter().map(VaultPath::as_str).collect();
            let text = format!(
                "no se deshizo: {} tiene cambios sin guardar; guárdalos antes de :tagundo",
                paths.join(", ")
            );
            self.last_tag_change = Some(last);
            return self.tell(Message::Error(text), cx);
        }
        let vault = self.session.read(cx).vault();
        let message = match vault.undo(&last.applied) {
            Ok(report) => {
                let back: Vec<Rename> = report
                    .moved_back
                    .iter()
                    .map(|rename| Rename {
                        from: rename.to.clone(),
                        to: rename.from.clone(),
                    })
                    .collect();
                self.follow_renames(&vault, back.iter(), cx);
                // Donde haya quedado cada archivo, aunque un renombre no se
                // haya podido deshacer.
                self.reread(&vault, last.applied.touched(), cx);
                // Solo si se deshizo todo: lo que cambió después sigue con el
                // tag nuevo, y la lista se quedaría sin esas notas.
                let whole = report.changed.is_empty()
                    && report.failed.is_empty()
                    && report.renames.is_empty();
                if let Some((from, to, choices)) = &last.list_followed
                    && whole
                    && self.list.read(cx).source_choices() == *choices
                {
                    self.list
                        .update(cx, |list, cx| list.rename_tag(to, from, cx));
                }
                undone(&last.title, &report)
            }
            Err(error) => {
                let text = format!("no se deshizo {}: {error}", last.title);
                self.last_tag_change = Some(last);
                Message::Error(text)
            }
        };
        self.tell(message, cx);
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
        self.workspace
            .read(cx)
            .unsaved_files()
            .filter_map(|(file, _)| session.vault_path(file))
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

    pub(super) fn tell(&mut self, message: Message, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            workspace.show_message(message);
            cx.notify();
        });
    }
}

fn undone(title: &str, report: &UndoReport) -> Message {
    let mut skipped: Vec<String> = report
        .changed
        .iter()
        .chain(report.failed.iter().map(|(path, _)| path))
        .map(|path| path.as_str().to_owned())
        .collect();
    skipped.extend(report.renames.iter().map(ToString::to_string));
    if skipped.is_empty() {
        Message::Info(format!("se deshizo {title}"))
    } else {
        let skipped = skipped.join(", ");
        Message::Error(format!(
            "se deshizo {title} salvo lo que cambió después o falló: {skipped}"
        ))
    }
}
