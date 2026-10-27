//! Renombrar o mover desde el panel sin romper links (03 §4): si el
//! renombre cambia links, la vista previa de esos cambios, con el renombre
//! fijo, antes de aplicarlos juntos con [`Vault::apply`]; si no cambia
//! ninguno, se renombra enseguida.

use std::fs;
use std::path::Path;

use gpui_kit::{AppContext as _, Context, Entity, Window};
use metalmind_vault::{
    ApplyError, Changes, RenameConflict, RenamePlan, UndoReport, Vault, VaultPath,
};

use super::WorkspaceView;
use super::overlay::Overlay;
use crate::change_preview::{ChangePreview, ChangePreviewEvent, ChangePreviewView, Notice};
use crate::workspace::Message;

impl WorkspaceView {
    pub(super) fn rename_entry(
        &mut self,
        from: &VaultPath,
        to: &VaultPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let vault = self.session.read(cx).vault();
        let plan = match vault.links().rename_plan(from, to) {
            Ok(plan) => plan,
            Err(error) => return self.rename_failed(error.to_string(), cx),
        };
        if plan.files.is_empty() && plan.conflicts.is_empty() {
            return self.rename_now(&vault, from, to, cx);
        }
        let changes = match on_whole_lines(plan.changes(), vault.root()) {
            Ok(changes) => changes,
            Err(error) => return self.rename_failed(error, cx),
        };
        let unsaved = self.unsaved_paths(cx);
        let Some(preview) = preview(&plan, changes, |path| unsaved.contains(path)) else {
            return self.rename_now(&vault, from, to, cx);
        };
        let view = cx.new(|cx| ChangePreviewView::new(preview, cx));
        let renamed = to.clone();
        let events = cx.subscribe_in(&view, window, move |this, view, event, window, cx| {
            this.on_rename_event(view, *event, &renamed, window, cx);
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

    /// Sin links que cambien: los documentos abiertos siguen al archivo.
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
            Ok(()) => self.reveal_renamed(to, cx),
            Err(error) => self.rename_failed(error.to_string(), cx),
        }
    }

    fn on_rename_event(
        &mut self,
        view: &Entity<ChangePreviewView>,
        event: ChangePreviewEvent,
        to: &VaultPath,
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
        self.focus_sidebar(window, cx);
        let Some((title, summary, changes)) = apply else {
            return;
        };
        let vault = self.session.read(cx).vault();
        match vault.apply(&changes) {
            Ok(applied) => {
                self.follow_renames(&vault, applied.renamed(), cx);
                self.reread(&vault, applied.touched(), cx);
                self.reveal_renamed(to, cx);
                // El resumen primero: la barra recorta lo que no cabe.
                self.tell(Message::Info(format!("{summary}; ahora es {to}")), cx);
            }
            Err(ApplyError::Interrupted { applied, cause }) if applied.is_empty() => {
                self.rename_failed(format!("{title}: no se cambió nada: {cause}"), cx);
            }
            // Los links ya apuntan a la ruta nueva y el renombre no se hizo:
            // se devuelven a como estaban.
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

    /// Al log y a la barra.
    fn rename_failed(&mut self, text: String, cx: &mut Context<Self>) {
        log::error!("{text}");
        self.tell(Message::Error(text), cx);
    }

    fn reveal_renamed(&mut self, to: &VaultPath, cx: &mut Context<Self>) {
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.reveal_entry(to.clone(), cx));
        self.sync_editors(cx);
    }
}

/// Cada edición de `changes` en sus líneas enteras, para ver el link en su
/// línea antes y después: lee cada archivo que se edita.
fn on_whole_lines(mut changes: Changes, root: &Path) -> Result<Changes, String> {
    for file in &mut changes.edits {
        let text = fs::read_to_string(root.join(file.path.as_str()))
            .map_err(|error| format!("no se pudo leer {}: {error}", file.path))?;
        *file = file.whole_lines(&text).map_err(|error| error.to_string())?;
    }
    Ok(changes)
}

/// La vista previa de `changes`, los de `plan`: los links que cambian, y el
/// renombre, fijo.
fn preview(
    plan: &RenamePlan,
    changes: Changes,
    unsaved: impl Fn(&VaultPath) -> bool,
) -> Option<ChangePreview> {
    let title = format!("Renombrar {} a {}", plan.from, plan.to);
    let notices = conflicts_notice(&plan.conflicts, &plan.to)
        .into_iter()
        .collect();
    ChangePreview::new(title, changes, notices, unsaved).map(ChangePreview::with_fixed_renames)
}

fn is_clean(report: &UndoReport) -> bool {
    report.changed.is_empty() && report.failed.is_empty() && report.renames.is_empty()
}

/// Lo que no se pudo devolver.
fn leftovers(report: &UndoReport) -> String {
    let files = report
        .changed
        .iter()
        .chain(report.failed.iter().map(|(path, _)| path));
    let mut left: Vec<String> = files.map(ToString::to_string).collect();
    left.extend(report.renames.iter().map(ToString::to_string));
    left.join(", ")
}

/// Cuántos links quedan rotos porque no se pueden reescribir, y dónde
/// están los primeros: un solo aviso, que no impide aplicar, por muchos
/// que sean.
fn conflicts_notice(conflicts: &[RenameConflict], to: &VaultPath) -> Option<Notice> {
    let (first, rest) = conflicts.split_first()?;
    let at = |conflict: &RenameConflict| format!("{}:{}", conflict.source, conflict.line + 1);
    let mut places: Vec<String> = std::iter::once(first)
        .chain(rest.iter().take(SHOWN_CONFLICTS - 1))
        .map(at)
        .collect();
    if conflicts.len() > SHOWN_CONFLICTS {
        places.push(format!("y {} más", conflicts.len() - SHOWN_CONFLICTS));
    }
    let links = if conflicts.len() == 1 {
        "un wikilink no puede".to_owned()
    } else {
        format!("{} wikilinks no pueden", conflicts.len())
    };
    Some(Notice {
        text: format!(
            "{links} nombrar {to} (lleva # o |), quedarán rotos: {}",
            places.join(", ")
        ),
        blocks: false,
    })
}

/// Cuántos lugares de links rotos nombra el aviso, como mucho.
const SHOWN_CONFLICTS: usize = 3;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use metalmind_vault::{EntryKind, LinkIndex};
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    /// La vista previa del renombre de `from` a `to` entre `notes`, con las
    /// ediciones en líneas enteras como las arma la ventana.
    fn preview_of(notes: &[(&str, &str)], from: &str, to: &str) -> ChangePreview {
        let plan = index(notes).rename_plan(&path(from), &path(to)).unwrap();
        let mut changes = plan.changes();
        for file in &mut changes.edits {
            let text = notes
                .iter()
                .find(|(name, _)| *name == file.path.as_str())
                .map(|(_, text)| *text)
                .unwrap();
            *file = file.whole_lines(text).unwrap();
        }
        preview(&plan, changes, |_| false).unwrap()
    }

    fn index(notes: &[(&str, &str)]) -> LinkIndex {
        let texts: HashMap<VaultPath, &str> = notes
            .iter()
            .map(|&(name, text)| (path(name), text))
            .collect();
        LinkIndex::build(
            texts.keys().map(|name| (name.clone(), EntryKind::Note)),
            |name| texts.get(name).map(|text| (*text).to_owned()),
        )
    }

    /// El renombre primero, elegido y fijo (quitarlo dejaría los links
    /// apuntando a la ruta vieja), y una fila por línea que cambia.
    #[test]
    fn los_links_que_cambian_y_el_renombre_fijo() {
        let notes = [
            ("a.md", "ver [[b]] y [[b#Meta]]\n"),
            ("b.md", "# B\n"),
            ("c.md", "[[b|la b]]\n"),
        ];
        let mut preview = preview_of(&notes, "b.md", "d.md");
        assert_eq!(preview.title(), "Renombrar b.md a d.md");
        assert_eq!(preview.selected(), 0, "el renombre, elegido");
        let rows: Vec<(&str, &str, &str)> = (0..preview.len())
            .map(|row| preview.row(row).unwrap())
            .map(|row| (row.path.as_str(), row.before, row.after))
            .collect();
        assert_eq!(
            rows,
            [
                ("b.md", "b.md", "d.md"),
                ("a.md", "ver [[b]] y [[b#Meta]]", "ver [[d]] y [[d#Meta]]"),
                ("c.md", "[[b|la b]]", "[[d|la b]]"),
            ]
        );
        assert!(preview.row(0).unwrap().fixed);
        assert!(!preview.row(1).unwrap().fixed);
        preview.toggle_selected();
        assert!(!preview.row(0).unwrap().dropped, "el renombre no se quita");
        preview.select(2);
        preview.toggle_selected();
        let changes = preview.changes();
        assert_eq!(changes.renames.len(), 1);
        assert_eq!(changes.edits.len(), 1, "solo a.md");
    }

    /// Un wikilink que no puede nombrar el destino se avisa sin impedir.
    #[test]
    fn un_link_que_no_se_puede_reescribir_se_avisa() {
        let notes = [("a.md", "uno\nver [[b]]\n"), ("b.md", "# B\n")];
        let preview = preview_of(&notes, "b.md", "b#2.md");
        assert_eq!(
            preview.notices(),
            [Notice {
                text: "un wikilink no puede nombrar b#2.md (lleva # o |), quedarán rotos: a.md:2"
                    .to_owned(),
                blocks: false,
            }]
        );
        assert_eq!(preview.blocked(), None);
    }

    /// Muchos links que no se pueden reescribir son un solo aviso, con los
    /// primeros lugares: si no, los avisos taparían la vista previa.
    #[test]
    fn muchos_links_rotos_son_un_solo_aviso() {
        let conflicts: Vec<RenameConflict> = (1..=5)
            .map(|at| RenameConflict {
                source: path(&format!("n{at}.md")),
                span: 0..5,
                line: at,
            })
            .collect();
        let notice = conflicts_notice(&conflicts, &path("b#2.md")).unwrap();
        assert_eq!(
            notice.text,
            "5 wikilinks no pueden nombrar b#2.md (lleva # o |), quedarán rotos: \
             n1.md:2, n2.md:3, n3.md:4, y 2 más"
        );
        assert!(!notice.blocks);
        assert_eq!(conflicts_notice(&[], &path("b.md")), None);
    }
}
