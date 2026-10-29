//! `:vaultundo` (también `:tagundo`): deshace el último cambio que la
//! ventana aplicó al vault, sea de tags o el renombre de una nota con sus
//! links (05 §5, 03 §4). Uno solo para todo: deshacer siempre es lo último
//! que se hizo, sin tener que recordar de qué tipo fue.

use std::collections::BTreeSet;

use gpui_kit::Context;
use metalmind_vault::{Applied, Rename, UndoReport, Vault, VaultPath};

use super::WorkspaceView;
use crate::workspace::Message;

/// El último cambio aplicado al vault, para deshacerlo.
#[derive(Default)]
pub(super) struct VaultUndo {
    last: Option<LastChange>,
}

struct LastChange {
    title: String,
    applied: Applied,
    list_followed: Option<ListFollowed>,
}

/// El tag del que la lista de notas pasó a otro, a cuál y cuántas fuentes
/// había elegido el usuario: deshacer el cambio la devuelve si no eligió
/// otra después.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ListFollowed {
    pub from: String,
    pub to: String,
    pub choices: u64,
}

/// Lo que deshizo [`VaultUndo::undo`].
#[derive(Debug)]
pub(super) struct Undone {
    title: String,
    pub applied: Applied,
    pub report: UndoReport,
    list_followed: Option<ListFollowed>,
}

impl VaultUndo {
    /// `applied`, llamado `title` en los avisos, pasa a ser lo que se
    /// deshace.
    pub fn remember(&mut self, title: String, applied: Applied) {
        self.last = Some(LastChange {
            title,
            applied,
            list_followed: None,
        });
    }

    /// La lista de notas siguió al último cambio.
    pub fn list_followed(&mut self, followed: ListFollowed) {
        if let Some(last) = &mut self.last {
            last.list_followed = Some(followed);
        }
    }

    /// Deshace el último cambio, salvo que un archivo que tocó tenga
    /// cambios sin guardar (`unsaved`): lo escribiría por debajo del buffer.
    /// Si no se deshace, sigue siendo el último.
    ///
    /// # Errors
    /// El aviso de por qué no se deshizo.
    pub fn undo(
        &mut self,
        vault: &Vault,
        unsaved: impl Fn(&VaultPath) -> bool,
    ) -> Result<Undone, Message> {
        let Some(last) = self.last.take() else {
            return Err(Message::Info(
                "no hay nada que deshacer: :vaultundo deshace el último renombre o cambio de tags"
                    .to_owned(),
            ));
        };
        let busy: BTreeSet<VaultPath> = last
            .applied
            .touched()
            .filter(|path| unsaved(path))
            .collect();
        if !busy.is_empty() {
            let paths: Vec<&str> = busy.iter().map(VaultPath::as_str).collect();
            let text = format!(
                "no se deshizo: {} tiene cambios sin guardar; guárdalos antes de :vaultundo",
                paths.join(", ")
            );
            self.last = Some(last);
            return Err(Message::Error(text));
        }
        match vault.undo(&last.applied) {
            Ok(report) => Ok(Undone {
                title: last.title,
                applied: last.applied,
                report,
                list_followed: last.list_followed,
            }),
            Err(error) => {
                let text = format!("no se deshizo {}: {error}", last.title);
                self.last = Some(last);
                Err(Message::Error(text))
            }
        }
    }
}

impl Undone {
    /// Los renombres deshechos, de la ruta nueva a la de antes.
    pub fn moved_back(&self) -> Vec<Rename> {
        self.report
            .moved_back
            .iter()
            .map(|rename| Rename {
                from: rename.to.clone(),
                to: rename.from.clone(),
            })
            .collect()
    }

    /// El tag al que hay que devolver la lista de notas, si la siguió y se
    /// deshizo todo: lo que cambió después sigue con el tag nuevo, y la
    /// lista se quedaría sin esas notas.
    pub fn list_back(&self) -> Option<&ListFollowed> {
        self.list_followed
            .as_ref()
            .filter(|_| is_clean(&self.report))
    }

    pub fn message(&self) -> Message {
        if is_clean(&self.report) {
            Message::Info(format!("se deshizo: {}", self.title))
        } else {
            Message::Error(format!(
                "se deshizo {} salvo lo que cambió después o falló: {}",
                self.title,
                leftovers(&self.report)
            ))
        }
    }
}

/// Si deshacer devolvió todo.
pub(super) fn is_clean(report: &UndoReport) -> bool {
    report.changed.is_empty() && report.failed.is_empty() && report.renames.is_empty()
}

/// Lo que no se pudo devolver.
pub(super) fn leftovers(report: &UndoReport) -> String {
    let files = report
        .changed
        .iter()
        .chain(report.failed.iter().map(|(path, _)| path));
    let mut left: Vec<String> = files.map(ToString::to_string).collect();
    left.extend(report.renames.iter().map(ToString::to_string));
    left.join(", ")
}

impl WorkspaceView {
    pub(super) fn undo_vault_change(&mut self, cx: &mut Context<Self>) {
        let unsaved = self.unsaved_paths(cx);
        let vault = self.session.read(cx).vault();
        let undone = match self.vault_undo.undo(&vault, |path| unsaved.contains(path)) {
            Ok(undone) => undone,
            Err(message) => return self.tell(message, cx),
        };
        self.follow_renames(&vault, undone.moved_back().iter(), cx);
        // Donde haya quedado cada archivo, aunque un renombre no se haya
        // podido deshacer.
        self.reread(&vault, undone.applied.touched(), cx);
        if let Some(back) = undone.list_back()
            && self.list.read(cx).source_choices() == back.choices
        {
            self.list
                .update(cx, |list, cx| list.rename_tag(&back.to, &back.from, cx));
        }
        self.tell(undone.message(), cx);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_vault::{Changes, FileEdits, TextEdit};
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    /// Un vault en disco con `notes`.
    fn vault(notes: &[(&str, &str)]) -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in notes {
            fs::write(dir.path().join(name), text).unwrap();
        }
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        (dir, vault)
    }

    fn read(dir: &tempfile::TempDir, name: &str) -> String {
        fs::read_to_string(dir.path().join(name)).unwrap()
    }

    /// Renombra `from` a `to` con sus links, como la vista previa al
    /// aplicarse entera, y lo guarda para deshacer.
    fn rename(undo: &mut VaultUndo, vault: &Vault, from: &str, to: &str) {
        let plan = vault.links().rename_plan(&path(from), &path(to)).unwrap();
        let applied = vault.apply(&plan.changes()).unwrap();
        undo.remember(format!("Renombrar {from} a {to}"), applied);
    }

    /// Renombrar una nota reescribe sus links; deshacer la devuelve a su
    /// nombre y los links a como estaban, y avisa qué deshizo.
    #[test]
    fn deshacer_un_renombre_devuelve_la_nota_y_sus_links() {
        let notes = [
            ("a.md", "ver [[b]] y [[b#Meta]]\n"),
            ("b.md", "# B\n"),
            ("c.md", "[x](./b.md)\n"),
        ];
        let (dir, vault) = vault(&notes);
        let mut undo = VaultUndo::default();
        rename(&mut undo, &vault, "b.md", "d.md");
        assert_eq!(read(&dir, "a.md"), "ver [[d]] y [[d#Meta]]\n");
        assert_eq!(read(&dir, "c.md"), "[x](./d.md)\n");
        assert!(!dir.path().join("b.md").exists());

        let undone = undo.undo(&vault, |_| false).unwrap();
        for (name, text) in notes {
            assert_eq!(read(&dir, name), text, "{name}");
        }
        assert!(!dir.path().join("d.md").exists());
        assert_eq!(
            undone.moved_back(),
            [Rename {
                from: path("d.md"),
                to: path("b.md"),
            }]
        );
        assert_eq!(
            undone.message(),
            Message::Info("se deshizo: Renombrar b.md a d.md".to_owned())
        );
    }

    /// Deshacer gasta el cambio: el siguiente `:vaultundo` no tiene nada y
    /// lo dice, en vez de no hacer nada callado.
    #[test]
    fn sin_nada_que_deshacer_lo_avisa() {
        let (_dir, vault) = vault(&[("a.md", "[[b]]\n"), ("b.md", "")]);
        let mut undo = VaultUndo::default();
        let nothing = Message::Info(
            "no hay nada que deshacer: :vaultundo deshace el último renombre o cambio de tags"
                .to_owned(),
        );
        assert_eq!(undo.undo(&vault, |_| false).unwrap_err(), nothing);
        rename(&mut undo, &vault, "b.md", "c.md");
        undo.undo(&vault, |_| false).unwrap();
        assert_eq!(undo.undo(&vault, |_| false).unwrap_err(), nothing);
    }

    /// Un archivo tocado con cambios sin guardar no se escribe por debajo
    /// del buffer: no se deshace nada y el cambio sigue ahí para después.
    #[test]
    fn un_buffer_sin_guardar_lo_detiene_sin_perderlo() {
        let (dir, vault) = vault(&[("a.md", "[[b]]\n"), ("b.md", "")]);
        let mut undo = VaultUndo::default();
        rename(&mut undo, &vault, "b.md", "c.md");
        let busy = undo
            .undo(&vault, |path| path.as_str() == "a.md")
            .unwrap_err();
        assert_eq!(
            busy,
            Message::Error(
                "no se deshizo: a.md tiene cambios sin guardar; guárdalos antes de :vaultundo"
                    .to_owned()
            )
        );
        assert_eq!(read(&dir, "a.md"), "[[c]]\n");
        undo.undo(&vault, |_| false).unwrap();
        assert_eq!(read(&dir, "a.md"), "[[b]]\n");
        assert!(dir.path().join("b.md").exists());
    }

    /// Lo que cambió después de aplicar no se pisa: se avisa, y la lista de
    /// notas no se devuelve a un tag que esas notas ya no tienen.
    #[test]
    fn lo_que_cambio_despues_se_queda_y_se_avisa() {
        let (dir, vault) = vault(&[("a.md", "[[b]]\n"), ("b.md", "")]);
        let mut undo = VaultUndo::default();
        rename(&mut undo, &vault, "b.md", "c.md");
        let followed = ListFollowed {
            from: "rust".to_owned(),
            to: "lang".to_owned(),
            choices: 1,
        };
        undo.list_followed(followed);
        fs::write(dir.path().join("a.md"), "[[c]] y más\n").unwrap();
        let undone = undo.undo(&vault, |_| false).unwrap();
        assert_eq!(read(&dir, "a.md"), "[[c]] y más\n");
        assert_eq!(
            undone.message(),
            Message::Error(
                "se deshizo Renombrar b.md a c.md salvo lo que cambió después o falló: a.md"
                    .to_owned()
            )
        );
        assert_eq!(undone.list_back(), None);
    }

    /// Un cambio de tags y un renombre comparten el deshacer: siempre se
    /// deshace el último, sea del tipo que sea.
    #[test]
    fn deshace_el_ultimo_cambio_sea_cual_sea() {
        let (dir, vault) = vault(&[("a.md", "#rust [[b]]\n"), ("b.md", "")]);
        let mut undo = VaultUndo::default();
        let mut tags = Changes::default();
        tags.edits.push(FileEdits {
            path: path("a.md"),
            edits: vec![TextEdit {
                range: 0..5,
                old: "#rust".to_owned(),
                new: "#lang".to_owned(),
                line: 0,
            }],
            base_len: None,
        });
        undo.remember(
            "Renombrar #rust a #lang".to_owned(),
            vault.apply(&tags).unwrap(),
        );
        rename(&mut undo, &vault, "b.md", "c.md");
        assert_eq!(read(&dir, "a.md"), "#lang [[c]]\n");
        let undone = undo.undo(&vault, |_| false).unwrap();
        assert_eq!(read(&dir, "a.md"), "#lang [[b]]\n", "solo el renombre");
        assert_eq!(
            undone.message(),
            Message::Info("se deshizo: Renombrar b.md a c.md".to_owned())
        );
    }
}
