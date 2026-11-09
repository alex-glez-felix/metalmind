//! `:vaultundo` (también `:tagundo`): deshace el último cambio que la
//! ventana aplicó al vault, sea de tags o el renombre de una nota con sus
//! links (05 §5, 03 §4). Uno solo para todo: deshacer siempre es lo último
//! que se hizo, sin tener que recordar de qué tipo fue.

use std::collections::BTreeSet;

use gpui_kit::Context;
use metalmind_keymap::WindowCommand;
use metalmind_vault::{Applied, FileEdits, Rename, UndoReport, Vault, VaultPath};

use super::VaultChanges;
use crate::workspace::{Message, MessageId, ToastAction};

/// El botón del aviso tras aplicar un cambio al vault.
const UNDO: ToastAction = ToastAction {
    label: "Deshacer",
    command: WindowCommand::UndoVaultChange,
};

/// El último cambio aplicado al vault, para deshacerlo.
#[derive(Default)]
pub(super) struct VaultUndo {
    last: Option<LastChange>,
    /// El aviso con "Deshacer" de `last`, que deja de valer al deshacerlo
    /// o al aplicar otro cambio.
    toast: Option<MessageId>,
}

struct LastChange {
    title: String,
    applied: Applied,
    /// Lo que devuelve a como estaban los buffers con cambios sin guardar
    /// que recibieron el cambio en vez del disco.
    buffers: Vec<FileEdits>,
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
    /// Lo que hay que devolver en los buffers que recibieron el cambio.
    pub buffers: Vec<FileEdits>,
    /// Los de `buffers` que no se pudieron devolver.
    pub left_in_buffers: Vec<VaultPath>,
    list_followed: Option<ListFollowed>,
}

impl VaultUndo {
    /// `applied`, llamado `title` en los avisos, pasa a ser lo que se
    /// deshace, con `buffers`, lo que devuelve los buffers que recibieron
    /// el cambio.
    pub fn remember(&mut self, title: String, applied: Applied, buffers: Vec<FileEdits>) {
        self.last = Some(LastChange {
            title,
            applied,
            buffers,
            list_followed: None,
        });
    }

    pub fn set_toast(&mut self, toast: MessageId) {
        self.toast = Some(toast);
    }

    pub fn take_toast(&mut self) -> Option<MessageId> {
        self.toast.take()
    }

    /// Lo que devolvería deshacer en los buffers que recibieron el último
    /// cambio.
    pub fn buffers(&self) -> &[FileEdits] {
        self.last.as_ref().map_or(&[], |last| &last.buffers)
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
                buffers: last.buffers,
                left_in_buffers: Vec::new(),
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
        self.list_followed.as_ref().filter(|_| self.is_clean())
    }

    fn is_clean(&self) -> bool {
        is_clean(&self.report) && self.left_in_buffers.is_empty()
    }

    pub fn message(&self) -> Message {
        if self.is_clean() {
            return Message::Info(format!("se deshizo: {}", self.title));
        }
        let mut left = leftovers(&self.report);
        for path in &self.left_in_buffers {
            if !left.is_empty() {
                left.push_str(", ");
            }
            left.push_str(path.as_str());
        }
        Message::Error(format!(
            "se deshizo {} salvo lo que cambió después o falló: {left}",
            self.title
        ))
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

impl VaultChanges {
    /// `:vaultundo`: deshace el último cambio y dice qué deshizo.
    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(typing) = self.typing_in_buffers(self.vault_undo.buffers(), cx) {
            let text =
                format!("no se deshizo: {typing} se está editando en Insert; sal de Insert antes");
            return self.tell(Message::Error(text), cx);
        }
        let unsaved = self.unsaved_paths(cx);
        let vault = self.session.read(cx).vault();
        let mut undone = match self.vault_undo.undo(&vault, |path| unsaved.contains(path)) {
            Ok(undone) => undone,
            Err(message) => return self.tell(message, cx),
        };
        if let Some(toast) = self.vault_undo.take_toast() {
            self.retire_toast(toast, cx);
        }
        undone.left_in_buffers = self.undo_buffers(&vault, &undone.buffers, cx);
        self.follow_renames(&vault, undone.moved_back().iter(), cx);
        // Donde haya quedado cada archivo, aunque un renombre no se haya
        // podido deshacer.
        self.reread(&vault, undone.applied.touched(), cx);
        Self::follow_saved_views(&undone.applied, cx);
        if let Some(back) = undone.list_back()
            && self.list.read(cx).source_choices() == back.choices
        {
            self.list
                .update(cx, |list, cx| list.rename_tag(&back.to, &back.from, cx));
        }
        self.tell(undone.message(), cx);
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
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_vault::{Changes, FileEdits, TextEdit};
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tag_change::{TagRequest, VaultConfigText};

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
        let plan = vault
            .snapshot()
            .links
            .rename_plan(&path(from), &path(to))
            .unwrap();
        let applied = vault.apply(&plan.changes()).unwrap();
        undo.remember(format!("Renombrar {from} a {to}"), applied, Vec::new());
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

    /// Renombrar un tag reescribe la vista guardada que lo nombra, y
    /// deshacerlo la devuelve con el resto del archivo intacto.
    #[test]
    fn deshacer_un_renombre_de_tag_devuelve_sus_vistas() {
        let (dir, vault) = vault(&[("a.md", "#gs\n")]);
        let config = "views:\n  estudio: \"#gs -#archivado\"  # mía\n  otra: \"#x\"\n";
        fs::create_dir(dir.path().join(".metalmind")).unwrap();
        fs::write(dir.path().join(".metalmind/config.yaml"), config).unwrap();
        let request = TagRequest::Rename {
            from: "gs".to_owned(),
            to: "estudio".to_owned(),
        };
        let preview = request
            .preview(
                &vault.snapshot().tags,
                &VaultConfigText::read(dir.path()),
                |_| false,
            )
            .unwrap()
            .unwrap();
        let mut undo = VaultUndo::default();
        undo.remember(
            request.title(),
            vault.apply(&preview.changes()).unwrap(),
            Vec::new(),
        );
        assert_eq!(read(&dir, "a.md"), "#estudio\n");
        assert_eq!(
            read(&dir, ".metalmind/config.yaml"),
            config.replace("\"#gs ", "\"#estudio ")
        );
        undo.undo(&vault, |_| false).unwrap();
        assert_eq!(read(&dir, "a.md"), "#gs\n");
        assert_eq!(read(&dir, ".metalmind/config.yaml"), config);
    }

    /// Lo que entró en buffers vuelve con el deshacer para devolverlo por el
    /// mismo camino; si no se pudo, se avisa y la lista no se devuelve.
    #[test]
    fn lo_de_los_buffers_vuelve_con_el_deshacer() {
        let (_dir, vault) = vault(&[("a.md", "#rust\n")]);
        let mut undo = VaultUndo::default();
        let back = FileEdits {
            path: path("b.md"),
            edits: vec![TextEdit {
                range: 0..5,
                old: "#lang".to_owned(),
                new: "#rust".to_owned(),
                line: 0,
            }],
            base_len: None,
        };
        let applied = vault.apply(&Changes::default()).unwrap();
        undo.remember(
            "Renombrar #rust a #lang".to_owned(),
            applied,
            vec![back.clone()],
        );
        assert_eq!(undo.buffers(), std::slice::from_ref(&back));
        undo.list_followed(ListFollowed {
            from: "rust".to_owned(),
            to: "lang".to_owned(),
            choices: 1,
        });
        let mut undone = undo.undo(&vault, |_| false).unwrap();
        assert_eq!(undone.buffers, [back]);
        assert!(undo.buffers().is_empty(), "ya no queda nada que deshacer");
        assert!(undone.list_back().is_some());
        undone.left_in_buffers = vec![path("b.md")];
        assert_eq!(undone.list_back(), None);
        assert_eq!(
            undone.message(),
            Message::Error(
                "se deshizo Renombrar #rust a #lang salvo lo que cambió después o falló: b.md"
                    .to_owned()
            )
        );
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
            Vec::new(),
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
