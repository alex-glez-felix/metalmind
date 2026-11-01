//! El panel de contexto sin GPUI (#164): los links de la nota activa en dos
//! pestañas, los que llegan a ella y los que salen, sobre el índice de links
//! del último lote del watcher.

pub mod backlinks;
pub mod outgoing;

use std::sync::Arc;

use metalmind_vault::{LinkIndex, VaultChange, VaultPath};

use self::backlinks::BacklinkOutline;
use self::outgoing::OutgoingOutline;
use crate::panel::{ClickOpen, Outline};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContextTab {
    /// Enlazan aquí.
    #[default]
    Backlinks,
    Outgoing,
}

impl ContextTab {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Backlinks => Self::Outgoing,
            Self::Outgoing => Self::Backlinks,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NoteContext {
    tab: ContextTab,
    pub backlinks: BacklinkOutline,
    pub outgoing: OutgoingOutline,
}

impl NoteContext {
    #[must_use]
    pub fn new(index: Arc<LinkIndex>) -> Self {
        Self {
            tab: ContextTab::default(),
            backlinks: BacklinkOutline::new(Arc::clone(&index)),
            outgoing: OutgoingOutline::new(index),
        }
    }

    #[must_use]
    pub fn tab(&self) -> ContextTab {
        self.tab
    }

    pub fn set_tab(&mut self, tab: ContextTab) {
        self.tab = tab;
    }

    /// La nota cuyo contexto se muestra: la del panel de edición enfocado,
    /// si es del vault.
    pub fn show_note(&mut self, note: Option<VaultPath>) {
        self.backlinks.show_note(note.clone());
        self.outgoing.show_note(note);
        self.refresh();
    }

    #[must_use]
    pub fn note(&self) -> Option<&VaultPath> {
        self.backlinks.note()
    }

    /// Un lote del watcher: lo elegido sigue a sus renombres y las filas
    /// salen del índice nuevo.
    pub fn apply(&mut self, changes: &[VaultChange], index: &Arc<LinkIndex>) {
        self.backlinks.follow(changes);
        self.outgoing.follow(changes);
        self.backlinks.set_index(Arc::clone(index));
        self.outgoing.set_index(Arc::clone(index));
        self.refresh();
    }

    fn refresh(&mut self) {
        self.backlinks.refresh();
        self.outgoing.refresh();
    }

    /// Cuántas notas enlazan aquí y cuántos destinos salen de aquí: lo que
    /// dice cada pestaña.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        (self.backlinks.note_count(), self.outgoing.rows().len())
    }

    #[must_use]
    pub fn outline(&self) -> &dyn Outline {
        match self.tab {
            ContextTab::Backlinks => &self.backlinks,
            ContextTab::Outgoing => &self.outgoing,
        }
    }

    pub fn outline_mut(&mut self) -> &mut dyn Outline {
        match self.tab {
            ContextTab::Backlinks => &mut self.backlinks,
            ContextTab::Outgoing => &mut self.outgoing,
        }
    }

    /// `l`: en una nota de los backlinks la abre o baja a sus links, como
    /// una carpeta; lo demás abre su fila. Devuelve qué abrir, si algo.
    pub fn expand_or_open(&mut self) -> Option<ClickOpen> {
        if self.tab == ContextTab::Backlinks && self.backlinks.is_note_selected() {
            self.backlinks.expand_or_enter();
            return None;
        }
        self.selected_open(true)
    }

    /// Lo que abre la fila elegida.
    #[must_use]
    pub fn selected_open(&self, focus_editor: bool) -> Option<ClickOpen> {
        let outline = self.outline();
        Some(ClickOpen {
            path: outline.selected_file()?.clone(),
            at: outline.selected_position(),
            focus_editor,
        })
    }

    /// Un clic en la fila `at`, el `count`-ésimo seguido: la elige y la
    /// abre; el doble clic le pasa el foco al editor.
    pub fn click(&mut self, at: usize, count: usize) -> Option<ClickOpen> {
        self.outline_mut().select_row(at);
        self.selected_open(count > 1)
    }
}

#[cfg(test)]
mod tests;
