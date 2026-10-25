//! El árbol de carpetas del panel lateral, sin GPUI: qué filas se ven,
//! cuál está seleccionada, qué carpetas están abiertas y el filtro.
//!
//! La estructura es la del [`VaultIndex`], que ya trae los hijos ordenados;
//! aquí solo vive el estado de la vista, que sigue a los renombres y
//! borrados que publica el watcher. Las filas se aplanan de nuevo tras cada
//! cambio y cuestan lo que las filas visibles, no lo que el vault.

mod filter;

use std::collections::BTreeSet;
use std::sync::Arc;

use metalmind_vault::{EntryKind, VaultChange, VaultIndex, VaultPath};

use self::filter::Matches;
use crate::panel::Outline;

/// Una fila visible del árbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub path: VaultPath,
    pub kind: EntryKind,
    /// 0 para lo que está en la raíz del vault.
    pub depth: usize,
    /// Una carpeta que muestra su contenido.
    pub expanded: bool,
    /// Offsets en bytes, dentro del nombre, de las letras que coinciden con
    /// el filtro.
    pub highlights: Vec<usize>,
}

impl Row {
    #[must_use]
    pub fn name(&self) -> &str {
        self.path.name()
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.kind == EntryKind::Folder
    }
}

#[derive(Clone, Debug)]
pub struct FileTree {
    index: Arc<VaultIndex>,
    expanded: BTreeSet<VaultPath>,
    selected: Option<VaultPath>,
    /// Lo que hay que seleccionar en cuanto el watcher lo traiga, como una
    /// nota recién creada desde el panel.
    reveal: Option<VaultPath>,
    filter: String,
    rows: Vec<Row>,
    selected_row: Option<usize>,
}

impl FileTree {
    /// Todo cerrado y la primera fila seleccionada.
    #[must_use]
    pub fn new(index: Arc<VaultIndex>) -> Self {
        Self::with_expanded(index, [])
    }

    /// Con esas carpetas abiertas, como las dejó la sesión anterior; las que
    /// ya no existen se ignoran.
    #[must_use]
    pub fn with_expanded(
        index: Arc<VaultIndex>,
        expanded: impl IntoIterator<Item = VaultPath>,
    ) -> Self {
        let mut tree = Self {
            index,
            expanded: expanded.into_iter().collect(),
            selected: None,
            reveal: None,
            filter: String::new(),
            rows: Vec::new(),
            selected_row: None,
        };
        tree.prune();
        tree.rebuild();
        tree.select_row(0);
        tree
    }

    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    #[must_use]
    pub fn selected(&self) -> Option<&VaultPath> {
        self.selected.as_ref()
    }

    #[must_use]
    pub fn selected_entry(&self) -> Option<&Row> {
        self.rows.get(self.selected_row?)
    }

    /// Las carpetas abiertas, para guardarlas al cerrar.
    pub fn expanded(&self) -> impl Iterator<Item = &VaultPath> {
        self.expanded.iter()
    }

    #[must_use]
    pub fn index(&self) -> &VaultIndex {
        &self.index
    }

    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    #[must_use]
    pub fn is_filtering(&self) -> bool {
        !self.filter.is_empty()
    }

    /// Selecciona `path`, abriendo las carpetas que lo contienen. Devuelve
    /// si quedó seleccionado: con un filtro que no lo muestra, no.
    pub fn select(&mut self, path: &VaultPath) -> bool {
        if !self.index.contains(path) || path.is_root() {
            return false;
        }
        if !self.is_filtering() {
            self.expanded
                .extend(path.ancestors().filter(|a| !a.is_root()));
            self.rebuild();
        }
        match self.rows.iter().position(|row| &row.path == path) {
            Some(row) => {
                self.select_row(row);
                true
            }
            None => false,
        }
    }

    /// Selecciona `path` ahora o, si todavía no está en el índice, cuando
    /// llegue con el watcher.
    pub fn reveal(&mut self, path: VaultPath) {
        if !self.select(&path) {
            self.reveal = Some(path);
        }
    }

    /// Abre o cierra la carpeta `path`. Con un filtro, todas se ven
    /// abiertas y esto no hace nada.
    pub fn toggle(&mut self, path: &VaultPath) {
        if self.expanded.contains(path) {
            self.collapse(path);
        } else {
            self.expand(path);
        }
    }

    pub fn expand(&mut self, path: &VaultPath) {
        if self.is_filtering() || self.index.kind(path) != Some(EntryKind::Folder) {
            return;
        }
        if self.expanded.insert(path.clone()) {
            self.rebuild();
        }
    }

    pub fn collapse(&mut self, path: &VaultPath) {
        if self.is_filtering() {
            return;
        }
        if self.expanded.remove(path) {
            self.rebuild();
        }
    }

    /// Filtra por `query` con la búsqueda difusa de rutas; vacío lo quita.
    /// Se ven las coincidencias y las carpetas que las contienen.
    pub fn set_filter(&mut self, query: &str) {
        if self.filter == query {
            return;
        }
        query.clone_into(&mut self.filter);
        self.refresh_keeping_selection();
        self.retry_reveal();
    }

    /// Aplica lo que publicó el watcher: `index` ya incluye `changes`, que
    /// vienen en un orden que se puede aplicar uno a uno. Las carpetas
    /// abiertas y la selección siguen a sus renombres; si lo seleccionado
    /// desaparece o queda dentro de una carpeta cerrada, queda seleccionada
    /// la fila que ocupaba su lugar.
    pub fn apply(&mut self, changes: &[VaultChange], index: Arc<VaultIndex>) {
        for change in changes {
            match change {
                VaultChange::Renamed { from, to, .. } => self.follow_rename(from, to),
                VaultChange::Removed { path, .. } => self.forget(path),
                VaultChange::Created { .. } | VaultChange::Modified { .. } => {}
            }
        }
        self.index = index;
        self.refresh_keeping_selection();
        self.retry_reveal();
    }

    /// Lo que esperaba a que el watcher lo trajera, ahora que las filas son
    /// las del índice y el filtro de este momento.
    fn retry_reveal(&mut self) {
        if let Some(path) = self.reveal.take() {
            self.reveal(path);
        }
    }

    fn follow_rename(&mut self, from: &VaultPath, to: &VaultPath) {
        let rebase = |path: &VaultPath| path.rebase(from, to).unwrap_or_else(|| path.clone());
        self.expanded = self.expanded.iter().map(rebase).collect();
        self.selected = self.selected.as_ref().map(rebase);
        self.reveal = self.reveal.as_ref().map(rebase);
    }

    fn forget(&mut self, removed: &VaultPath) {
        self.expanded.retain(|path| !path.starts_with(removed));
        let gone = |path: &Option<VaultPath>| path.as_ref().is_some_and(|p| p.starts_with(removed));
        if gone(&self.selected) {
            self.selected = None;
        }
        if gone(&self.reveal) {
            self.reveal = None;
        }
    }

    /// Olvida las carpetas abiertas que ya no son carpetas del índice.
    fn prune(&mut self) {
        let index = &self.index;
        self.expanded
            .retain(|path| index.kind(path) == Some(EntryKind::Folder));
    }

    /// Rehace las filas; la selección se queda en su ruta si sigue a la
    /// vista y, si no, en la fila que ocupaba.
    fn refresh_keeping_selection(&mut self) {
        let previous_row = self.selected_row;
        self.rebuild();
        let found = self
            .selected
            .as_ref()
            .and_then(|path| self.rows.iter().position(|row| &row.path == path));
        match (found, previous_row) {
            (Some(row), _) | (None, Some(row)) => self.select_row(row),
            (None, None) => self.select_row(0),
        }
    }

    fn rebuild(&mut self) {
        let matches = (!self.filter.is_empty()).then(|| Matches::find(&self.index, &self.filter));
        let mut rows = Vec::with_capacity(self.rows.len());
        self.push_children(&VaultPath::root(), 0, matches.as_ref(), &mut rows);
        self.rows = rows;
        self.selected_row = self
            .selected
            .as_ref()
            .and_then(|path| self.rows.iter().position(|row| &row.path == path));
    }

    fn push_children(
        &self,
        dir: &VaultPath,
        depth: usize,
        matches: Option<&Matches>,
        rows: &mut Vec<Row>,
    ) {
        for entry in self.index.children(dir) {
            let highlights = match matches {
                Some(matches) => match matches.get(entry.path) {
                    Some(highlights) => highlights.to_vec(),
                    None => continue,
                },
                None => Vec::new(),
            };
            let is_folder = entry.kind == EntryKind::Folder;
            let expanded = is_folder
                && match matches {
                    Some(matches) => matches.has_inside(entry.path),
                    None => self.expanded.contains(entry.path),
                };
            rows.push(Row {
                path: entry.path.clone(),
                kind: entry.kind,
                depth,
                expanded,
                highlights,
            });
            if expanded {
                self.push_children(entry.path, depth + 1, matches, rows);
            }
        }
    }
}

impl Outline for FileTree {
    fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }

    fn select_row(&mut self, row: usize) {
        let Some(last) = self.rows.len().checked_sub(1) else {
            self.selected = None;
            self.selected_row = None;
            return;
        };
        let row = row.min(last);
        self.selected = Some(self.rows[row].path.clone());
        self.selected_row = Some(row);
    }

    /// `h`: cierra la carpeta seleccionada o, si ya está cerrada o es un
    /// archivo, sube a la carpeta que la contiene.
    fn collapse_or_parent(&mut self) {
        let Some(row) = self.selected_entry() else {
            return;
        };
        if row.expanded && !self.is_filtering() {
            let path = row.path.clone();
            self.collapse(&path);
            return;
        }
        if let Some(parent) = row.path.parent().filter(|parent| !parent.is_root()) {
            self.select(&parent);
        }
    }

    /// `l` sobre una carpeta: la abre o, si ya está abierta, baja a su
    /// primer hijo.
    fn expand_or_enter(&mut self) {
        let (Some(row), Some(at)) = (self.selected_entry(), self.selected_row) else {
            return;
        };
        if !row.is_folder() {
            return;
        }
        if row.expanded {
            let depth = row.depth;
            if self.rows.get(at + 1).is_some_and(|next| next.depth > depth) {
                self.select_row(at + 1);
            }
        } else {
            let path = row.path.clone();
            self.expand(&path);
        }
    }

    fn toggle_selected(&mut self) {
        if let Some(row) = self.selected_entry().filter(|row| row.is_folder()) {
            let path = row.path.clone();
            self.toggle(&path);
        }
    }

    fn selected_path(&self) -> Option<&VaultPath> {
        self.selected.as_ref()
    }

    fn selected_file(&self) -> Option<&VaultPath> {
        self.selected_entry()
            .filter(|row| !row.is_folder())
            .map(|row| &row.path)
    }
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
