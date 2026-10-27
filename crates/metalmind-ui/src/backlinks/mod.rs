//! La vista de backlinks del panel lateral, sin GPUI: los links que llegan a
//! la nota activa, agrupados por la nota donde están, cada uno con su línea.
//! Sale de `LinkIndex::backlinks`, que es buscar en un mapa: se rehace
//! cuando cambia la nota activa o llega otro índice del watcher.

use std::collections::BTreeSet;
use std::sync::Arc;

use metalmind_vault::{LinkContext, LinkIndex, VaultChange, VaultPath};

use crate::panel::Outline;
use crate::workspace::LinePosition;

/// Qué es una fila: una nota con links a la activa, o uno de esos links,
/// por la nota donde está y su byte en ella.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RowId {
    Note(VaultPath),
    Link(VaultPath, usize),
}

/// Una fila: una nota con links a la activa, o uno de esos links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Note {
        source: VaultPath,
        /// Cuántos de sus links llegan a la activa.
        count: usize,
        expanded: bool,
        /// Dónde está el primero.
        first: LinePosition,
    },
    Link {
        source: VaultPath,
        /// El byte donde empieza en `source`.
        offset: usize,
        at: LinePosition,
        context: LinkContext,
    },
}

impl Row {
    #[must_use]
    pub fn id(&self) -> RowId {
        match self {
            Self::Note { source, .. } => RowId::Note(source.clone()),
            Self::Link { source, offset, .. } => RowId::Link(source.clone(), *offset),
        }
    }

    #[must_use]
    pub fn source(&self) -> &VaultPath {
        match self {
            Self::Note { source, .. } | Self::Link { source, .. } => source,
        }
    }

    /// Dónde abre la fila su nota: en el link, o en el primero de la nota.
    #[must_use]
    pub fn position(&self) -> LinePosition {
        match self {
            Self::Note { first, .. } => *first,
            Self::Link { at, .. } => *at,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BacklinkOutline {
    index: Arc<LinkIndex>,
    note: Option<VaultPath>,
    /// Las notas cerradas; las demás se ven abiertas.
    collapsed: BTreeSet<VaultPath>,
    selected: Option<RowId>,
    rows: Vec<Row>,
    selected_row: Option<usize>,
    /// Llegó otro índice o cambió la nota desde que se armaron las filas.
    stale: bool,
}

impl BacklinkOutline {
    #[must_use]
    pub fn new(index: Arc<LinkIndex>) -> Self {
        Self {
            index,
            note: None,
            collapsed: BTreeSet::new(),
            selected: None,
            rows: Vec::new(),
            selected_row: None,
            stale: true,
        }
    }

    /// La nota cuyos backlinks se muestran: la del panel de edición
    /// enfocado, si es del vault. Otra nota empieza en su primera fila.
    pub fn show_note(&mut self, note: Option<VaultPath>) {
        if self.note != note {
            self.note = note;
            self.selected = None;
            self.selected_row = None;
            self.stale = true;
        }
    }

    #[must_use]
    pub fn note(&self) -> Option<&VaultPath> {
        self.note.as_ref()
    }

    /// El índice de links de otro lote del watcher.
    pub fn set_index(&mut self, index: Arc<LinkIndex>) {
        if !Arc::ptr_eq(&self.index, &index) {
            self.index = index;
            self.stale = true;
        }
    }

    /// Tras un lote del watcher: la nota activa, la elegida y las cerradas
    /// siguen a sus renombres.
    pub fn follow(&mut self, changes: &[VaultChange]) {
        for change in changes {
            let VaultChange::Renamed { from, to, .. } = change else {
                continue;
            };
            let moved = |path: &mut VaultPath| {
                if let Some(rebased) = path.rebase(from, to) {
                    *path = rebased;
                }
            };
            if let Some(note) = &mut self.note {
                moved(note);
            }
            if let Some(RowId::Note(source) | RowId::Link(source, _)) = &mut self.selected {
                moved(source);
            }
            self.collapsed = std::mem::take(&mut self.collapsed)
                .into_iter()
                .map(|mut source| {
                    moved(&mut source);
                    source
                })
                .collect();
        }
    }

    /// Rehace las filas si cambió algo. La elegida se queda en su fila si
    /// sigue, y si no en la que ocupaba.
    pub fn refresh(&mut self) {
        if !self.stale {
            return;
        }
        self.stale = false;
        let previous_row = self.selected_row;
        self.rebuild();
        match (self.selected_row, previous_row) {
            (Some(row), _) | (None, Some(row)) => self.select_row(row),
            (None, None) => self.select_row(0),
        }
    }

    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Qué fila está elegida, aunque las filas todavía no se hayan rehecho.
    #[must_use]
    pub fn selected_id(&self) -> Option<&RowId> {
        self.selected.as_ref()
    }

    #[must_use]
    pub fn selected_entry(&self) -> Option<&Row> {
        self.rows.get(self.selected_row?)
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        let Some(note) = &self.note else {
            self.selected_row = None;
            return;
        };
        let mut links = self.index.backlinks(note).peekable();
        while let Some(first) = links.next() {
            let source = first.source;
            let mut group = vec![first.link];
            while let Some(next) = links.next_if(|next| next.source == source) {
                group.push(next.link);
            }
            let expanded = !self.collapsed.contains(source);
            self.rows.push(Row::Note {
                source: source.clone(),
                count: group.len(),
                expanded,
                first: position(group[0]),
            });
            if expanded {
                self.rows.extend(group.into_iter().map(|link| Row::Link {
                    source: source.clone(),
                    offset: link.span.start,
                    at: position(link),
                    context: link.context.clone(),
                }));
            }
        }
        self.selected_row = self
            .selected
            .as_ref()
            .and_then(|id| self.rows.iter().position(|row| row.id() == *id));
    }

    /// Pliega o despliega la nota de la fila `row` y deja elegida su fila.
    fn set_expanded(&mut self, row: usize, expanded: bool) {
        let Some(source) = self.rows.get(row).map(|row| row.source().clone()) else {
            return;
        };
        if expanded {
            self.collapsed.remove(&source);
        } else {
            self.collapsed.insert(source.clone());
        }
        self.selected = Some(RowId::Note(source));
        self.rebuild();
    }

    fn is_expanded(&self, row: usize) -> bool {
        matches!(self.rows.get(row), Some(Row::Note { expanded: true, .. }))
    }
}

fn position(link: &metalmind_vault::IndexedLink) -> LinePosition {
    LinePosition {
        line: link.line,
        column: link.column,
    }
}

impl Outline for BacklinkOutline {
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
        self.selected = Some(self.rows[row].id());
        self.selected_row = Some(row);
    }

    /// En un link, sube a su nota; en una nota abierta, la cierra.
    fn collapse_or_parent(&mut self) {
        let Some(row) = self.selected_row else {
            return;
        };
        match &self.rows[row] {
            Row::Link { source, .. } => {
                let note = RowId::Note(source.clone());
                if let Some(parent) = self.rows.iter().position(|row| row.id() == note) {
                    self.select_row(parent);
                }
            }
            Row::Note { expanded: true, .. } => self.set_expanded(row, false),
            Row::Note { .. } => {}
        }
    }

    /// En una nota cerrada, la abre; en una abierta, baja a su primer link.
    fn expand_or_enter(&mut self) {
        let Some(row) = self.selected_row else {
            return;
        };
        if !matches!(self.rows[row], Row::Note { .. }) {
            return;
        }
        if self.is_expanded(row) {
            self.select_row(row + 1);
        } else {
            self.set_expanded(row, true);
        }
    }

    fn toggle_selected(&mut self) {
        if let Some(row) = self.selected_row
            && matches!(self.rows[row], Row::Note { .. })
        {
            self.set_expanded(row, !self.is_expanded(row));
        }
    }

    fn selected_path(&self) -> Option<&VaultPath> {
        self.selected_entry().map(Row::source)
    }

    /// Toda fila abre su nota: la de un link, en el link.
    fn selected_file(&self) -> Option<&VaultPath> {
        self.selected_path()
    }

    fn selected_position(&self) -> Option<LinePosition> {
        self.selected_entry().map(Row::position)
    }
}

#[cfg(test)]
mod tests;
