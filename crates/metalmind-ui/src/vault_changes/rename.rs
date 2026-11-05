//! Renombrar o mover desde el panel sin romper links (03 §4): si el renombre
//! cambia links, la vista previa de esos cambios, con el renombre fijo, antes
//! de aplicarlos juntos con [`Vault::apply`]; si no cambia ninguno, se renombra
//! enseguida.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, WeakEntity};
use metalmind_vault::{ApplyError, LinkIndex, Vault, VaultPath};

use super::rename_plan::{Arrival, RenameBuild, RenamePreview, arrival, build, preview, title};
use super::undo::{is_clean, leftovers};
use super::{Accepted, PreviewFlow, VaultChanges, VaultChangesEvent};
use crate::change_preview::ChangePreviewView;
use crate::workspace::Message;

/// Un renombre con su plan armado, a la espera de [`VaultChanges::finish_rename`].
pub(super) struct PlannedRename {
    index: Arc<LinkIndex>,
    from: VaultPath,
    to: VaultPath,
    built: Result<RenameBuild, String>,
}

impl VaultChanges {
    /// Arma el plan fuera del hilo de la interfaz (lee cada archivo que se
    /// edita) y avisa con [`VaultChangesEvent::RenamePlanned`].
    pub fn rename(&mut self, from: &VaultPath, to: &VaultPath, cx: &mut Context<Self>) {
        let vault = self.session.read(cx).vault();
        let (index, root) = (vault.links(), vault.root().to_owned());
        let (from, to) = (from.clone(), to.clone());
        self.planned_rename = None;
        self.renaming = Some(cx.spawn(async move |changes, cx| {
            let building = (Arc::clone(&index), root, from.clone(), to.clone());
            let built = cx
                .background_executor()
                .spawn(async move { build(&building.0, &building.1, &building.2, &building.3) })
                .await;
            // Si la ventana se cerró, no hay nada que mostrar.
            let _ = changes.update(cx, |changes, cx| {
                changes.renaming = None;
                changes.planned_rename = Some(PlannedRename {
                    index,
                    from,
                    to,
                    built,
                });
                cx.emit(VaultChangesEvent::RenamePlanned);
            });
        }));
    }

    /// Sigue con el plan armado: renombra o muestra su vista previa, salvo que
    /// la ventana tenga otra cosa abierta (`overlay_open`), a la que no se le
    /// pasa por encima.
    pub fn finish_rename(&mut self, overlay_open: bool, cx: &mut Context<Self>) {
        let Some(PlannedRename {
            index,
            from,
            to,
            built,
        }) = self.planned_rename.take()
        else {
            return;
        };
        let vault = self.session.read(cx).vault();
        match arrival(&index, &vault.links(), overlay_open) {
            Arrival::Show => {}
            Arrival::Rebuild => return self.rename(&from, &to, cx),
            Arrival::Busy => {
                let text = format!(
                    "no se renombró {from}: se abrió otra ventana antes de su vista previa"
                );
                return self.tell(Message::Info(text), cx);
            }
        }
        let built = match built {
            Ok(built) => built,
            Err(error) => return self.rename_failed(error, cx),
        };
        if built.plan.files.is_empty() && built.plan.conflicts.is_empty() {
            return self.rename_now(&vault, &from, &to, cx);
        }
        let unsaved = self.unsaved_paths(cx);
        let Some(preview) = preview(&built.plan, built.changes, |path| unsaved.contains(path))
        else {
            return self.rename_now(&vault, &from, &to, cx);
        };
        let view = cx.new(|cx| ChangePreviewView::new(preview, cx));
        let flow = PreviewFlow::Rename(RenamePreview::new(from, to, index));
        self.open_preview(view, flow, cx);
    }

    /// Con la vista previa de un renombre abierta, un lote del vault que
    /// trae otro índice de links (como el autoguardado de un archivo que la
    /// bloqueaba) la rehace con los textos de ahora: si no, aplicar
    /// encontraría los archivos cambiados o dejaría fuera un link nuevo.
    /// Mientras tanto no se aplica. Lo quitado sigue quitado.
    pub(super) fn refresh_rename_preview(
        &mut self,
        latest: &Arc<LinkIndex>,
        cx: &mut Context<Self>,
    ) {
        let root = self.session.read(cx).root().to_owned();
        let Some((_, rename)) = self.open_rename() else {
            return;
        };
        if !rename.fall_behind(latest) {
            return;
        }
        let (from, to) = rename.paths();
        let building = (Arc::clone(latest), root, from.clone(), to.clone());
        let latest = Arc::clone(latest);
        // Reemplazar la tarea cancela la del índice anterior, si no terminó.
        rename.set_rebuild(cx.spawn(async move |changes: WeakEntity<Self>, cx| {
            let built = cx
                .background_executor()
                .spawn(async move { build(&building.0, &building.1, &building.2, &building.3) })
                .await;
            let _ = changes.update(cx, |changes, cx| {
                changes.show_rebuilt(&latest, built, cx);
            });
        }));
    }

    /// La vista previa de un renombre, si sigue en la ventana.
    pub(super) fn open_rename(
        &mut self,
    ) -> Option<(Entity<ChangePreviewView>, &mut RenamePreview)> {
        let open = self.preview.as_mut()?;
        let view = open.view.upgrade()?;
        match &mut open.flow {
            PreviewFlow::Rename(rename) => Some((view, rename)),
            PreviewFlow::Tags(_) => None,
        }
    }

    /// Pone la vista previa rehecha para `built_for`, si sigue abierta para
    /// ese índice; si no se pudo rehacer, deja la que había, sin aplicarse
    /// hasta que el próximo lote la rehaga.
    fn show_rebuilt(
        &mut self,
        built_for: &Arc<LinkIndex>,
        built: Result<RenameBuild, String>,
        cx: &mut Context<Self>,
    ) {
        let unsaved = self.unsaved_paths(cx);
        let Some((view, rename)) = self.open_rename() else {
            return;
        };
        let rebuilt = built.and_then(|built| {
            preview(&built.plan, built.changes, |path| unsaved.contains(path))
                .ok_or_else(|| "ya no hay nada que renombrar".to_owned())
        });
        match rebuilt {
            Ok(mut fresh) => {
                if !rename.catch_up(built_for) {
                    return;
                }
                view.update(cx, |view, cx| {
                    fresh.carry_over(view.preview());
                    *view.preview_mut() = fresh;
                    cx.notify();
                });
            }
            // Un archivo que cambió entre el índice y la lectura: el próximo
            // lote trae otro índice y la rehace.
            Err(error) => log::warn!("no se rehízo la vista previa del renombre: {error}"),
        }
    }

    /// Sin links que cambien: los documentos abiertos siguen al archivo, y
    /// `:vaultundo` lo deshace como a cualquier renombre.
    fn rename_now(
        &mut self,
        vault: &Vault,
        from: &VaultPath,
        to: &VaultPath,
        cx: &mut Context<Self>,
    ) {
        let renamed = self.workspace.update(cx, |workspace, cx| {
            cx.notify();
            workspace.rename_file(vault, from, to)
        });
        match renamed {
            Ok(applied) => {
                self.remember_change(title(from, to), applied, Vec::new(), cx);
                cx.emit(VaultChangesEvent::Renamed(to.clone()));
            }
            // Solo hay un renombre: si se paró, no se hizo nada y basta su causa.
            Err(ApplyError::Interrupted { cause, .. }) => {
                self.rename_failed(cause.to_string(), cx);
            }
            Err(error) => self.rename_failed(error.to_string(), cx),
        }
    }

    /// Aplica al disco lo que quedó en la vista previa de `rename`. Si se
    /// para a medias, devuelve lo escrito: los links ya apuntarían a una ruta
    /// que no existe.
    pub(super) fn apply_rename(
        &mut self,
        rename: &RenamePreview,
        accepted: Accepted,
        cx: &mut Context<Self>,
    ) {
        let Accepted {
            title,
            summary,
            changes,
        } = accepted;
        let vault = self.session.read(cx).vault();
        match vault.apply(&changes) {
            Ok(applied) => {
                self.follow_renames(&vault, applied.renamed(), cx);
                self.reread(&vault, applied.touched(), cx);
                self.remember_change(title, applied, Vec::new(), cx);
                cx.emit(VaultChangesEvent::Renamed(rename.paths().1.clone()));
                self.tell_undoable(summary, cx);
            }
            Err(ApplyError::Interrupted { applied, cause }) if applied.is_empty() => {
                self.rename_failed(format!("{title}: no se cambió nada: {cause}"), cx);
            }
            Err(ApplyError::Interrupted { applied, cause }) => {
                let back = vault.undo(&applied);
                self.reread(&vault, applied.touched(), cx);
                let text = match back {
                    Ok(report) if is_clean(&report) => {
                        format!("{title}: no se pudo ({cause}); los links quedaron como estaban")
                    }
                    Ok(report) => format!(
                        "{title}: no se pudo ({cause}) y no se pudo devolver: {}",
                        leftovers(&report)
                    ),
                    Err(error) => format!("{title}: no se pudo ({cause}) ni deshacer: {error}"),
                };
                self.rename_failed(text, cx);
            }
            Err(error) => self.rename_failed(format!("{title}: no se cambió nada: {error}"), cx),
        }
    }

    /// Al log y a un toast.
    fn rename_failed(&mut self, text: String, cx: &mut Context<Self>) {
        log::error!("{text}");
        self.tell(Message::Error(text), cx);
    }
}
