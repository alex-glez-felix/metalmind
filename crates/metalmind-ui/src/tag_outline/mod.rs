//! La vista de tags del panel lateral, sin GPUI (05 §3): el [`TagTree`] del
//! vault aplanado en filas, con lo que está abierto y lo seleccionado.
//!
//! Un tag con varios padres sale bajo cada uno, así que una fila se
//! identifica por su camino desde la raíz ([`RowId`]), no por su tag: abrir
//! `#gpui` bajo `#rust` no lo abre bajo `#ui`. Las filas se rehacen con
//! cada lote del watcher y cuestan lo que las filas visibles.

use std::collections::BTreeSet;
use std::sync::Arc;

use metalmind_vault::tags::{Scope, TagColor, TagIcon, TagKey, TagTree, Tags};
use metalmind_vault::{VaultChange, VaultPath};

use crate::panel::Outline;

/// Dónde está una fila: el camino de tags desde la raíz hasta ella.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RowId {
    Tag(Vec<TagKey>),
    /// Una nota bajo el último tag del camino.
    Note(Vec<TagKey>, VaultPath),
    Inbox,
    InboxNote(VaultPath),
}

/// Una fila visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: RowId,
    /// 0 para los tags raíz y el buzón.
    pub depth: usize,
    pub kind: RowKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    Tag {
        /// La forma establecida (05 §6).
        name: String,
        count: usize,
        color: Option<TagColor>,
        icon: Option<TagIcon>,
        /// Tiene tags hijos o notas que mostrar.
        expandable: bool,
        expanded: bool,
        /// Ya está en el camino desde la raíz: no se abre otra vez.
        cycle: bool,
    },
    Note,
    Inbox {
        count: usize,
        expanded: bool,
    },
}

impl Row {
    /// La forma establecida del tag de la fila, si es uno.
    #[must_use]
    pub fn tag_name(&self) -> Option<&str> {
        match &self.kind {
            RowKind::Tag { name, .. } => Some(name),
            RowKind::Note | RowKind::Inbox { .. } => None,
        }
    }

    /// La nota de la fila, si es una.
    #[must_use]
    pub fn note(&self) -> Option<&VaultPath> {
        match &self.id {
            RowId::Note(_, path) | RowId::InboxNote(path) => Some(path),
            RowId::Tag(_) | RowId::Inbox => None,
        }
    }

    #[must_use]
    pub fn is_expanded(&self) -> bool {
        match self.kind {
            RowKind::Tag { expanded, .. } | RowKind::Inbox { expanded, .. } => expanded,
            RowKind::Note => false,
        }
    }

    fn can_expand(&self) -> bool {
        match self.kind {
            RowKind::Tag { expandable, .. } => expandable,
            RowKind::Inbox { count, .. } => count > 0,
            RowKind::Note => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TagOutline {
    tags: Arc<Tags>,
    tree: TagTree,
    /// Los tags que llegaron y todavía no están en las filas.
    pending: Option<Arc<Tags>>,
    expanded: BTreeSet<RowId>,
    selected: Option<RowId>,
    rows: Vec<Row>,
    selected_row: Option<usize>,
}

impl TagOutline {
    /// Sin filas hasta [`TagOutline::refresh`]: armar el árbol cuesta lo que
    /// el vault, y la vista puede no mostrarse nunca. Después, todo cerrado
    /// y la primera fila seleccionada.
    #[must_use]
    pub fn new(tags: Arc<Tags>) -> Self {
        Self {
            tags: Arc::clone(&tags),
            tree: TagTree::default(),
            pending: Some(tags),
            expanded: BTreeSet::new(),
            selected: None,
            rows: Vec::new(),
            selected_row: None,
        }
    }

    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    #[must_use]
    pub fn selected_entry(&self) -> Option<&Row> {
        self.rows.get(self.selected_row?)
    }

    /// Los tags de otro lote del watcher. Las filas no cambian hasta
    /// [`TagOutline::refresh`]: armar el árbol cuesta lo que el vault, y no
    /// hace falta mientras la vista no se ve.
    /// Los tags del último lote del watcher.
    #[must_use]
    pub fn tags(&self) -> &Tags {
        &self.tags
    }

    pub fn set_tags(&mut self, tags: Arc<Tags>) {
        self.pending = Some(tags);
    }

    /// Tras un lote del watcher, antes de sus tags: la nota seleccionada
    /// sigue a sus renombres, así su fila sigue elegida aunque cambie de
    /// lugar.
    pub fn follow(&mut self, changes: &[VaultChange]) {
        let Some(RowId::Note(_, path) | RowId::InboxNote(path)) = &mut self.selected else {
            return;
        };
        for change in changes {
            if let VaultChange::Renamed { from, to, .. } = change
                && let Some(moved) = path.rebase(from, to)
            {
                *path = moved;
            }
        }
    }

    /// Rehace las filas con los últimos tags, si llegaron otros. Lo abierto
    /// sigue abierto y la selección se queda en su fila si sigue, y si no en
    /// la que ocupaba.
    pub fn refresh(&mut self) {
        let Some(tags) = self.pending.take() else {
            return;
        };
        self.tree = tags.tree(&Scope::default().filter(&tags));
        self.tags = tags;
        let previous_row = self.selected_row;
        self.rebuild();
        match (self.selected_row, previous_row) {
            (Some(row), _) | (None, Some(row)) => self.select_row(row),
            (None, None) => self.select_row(0),
        }
    }

    fn rebuild(&mut self) {
        let mut rows = Vec::with_capacity(self.rows.len());
        for root in self.tree.roots() {
            self.push_tag(std::slice::from_ref(root), &mut rows);
        }
        let inbox = self.tree.inbox();
        let expanded = !inbox.is_empty() && self.expanded.contains(&RowId::Inbox);
        rows.push(Row {
            id: RowId::Inbox,
            depth: 0,
            kind: RowKind::Inbox {
                count: inbox.len(),
                expanded,
            },
        });
        if expanded {
            rows.extend(inbox.iter().map(|path| Row {
                id: RowId::InboxNote(path.clone()),
                depth: 1,
                kind: RowKind::Note,
            }));
        }
        self.rows = rows;
        self.selected_row = self
            .selected
            .as_ref()
            .and_then(|id| self.rows.iter().position(|row| &row.id == id));
    }

    /// El tag al final de `route` y, si está abierto, sus hijos y sus notas.
    fn push_tag(&self, route: &[TagKey], rows: &mut Vec<Row>) {
        let Some((key, ancestors)) = route.split_last() else {
            return;
        };
        let Some(node) = self.tree.node(key) else {
            return;
        };
        let cycle = ancestors.contains(key);
        let expandable = !cycle && (!node.children.is_empty() || node.count > 0);
        let id = RowId::Tag(route.to_vec());
        let expanded = expandable && self.expanded.contains(&id);
        rows.push(Row {
            id,
            depth: ancestors.len(),
            kind: RowKind::Tag {
                name: node.name.clone(),
                count: node.count,
                color: node.color.clone(),
                icon: node.icon.clone(),
                expandable,
                expanded,
                cycle,
            },
        });
        if !expanded {
            return;
        }
        for child in &node.children {
            let mut child_route = route.to_vec();
            child_route.push(child.clone());
            self.push_tag(&child_route, rows);
        }
        let notes = self
            .tags
            .tagged_notes(key, &Scope::default().filter(&self.tags));
        rows.extend(notes.into_iter().map(|path| Row {
            id: RowId::Note(route.to_vec(), path),
            depth: route.len(),
            kind: RowKind::Note,
        }));
    }
}

impl Outline for TagOutline {
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
        self.selected = Some(self.rows[row].id.clone());
        self.selected_row = Some(row);
    }

    /// Abre o cierra la fila seleccionada, si es un tag o el buzón.
    fn toggle_selected(&mut self) {
        let Some(row) = self.selected_entry() else {
            return;
        };
        if !row.can_expand() {
            return;
        }
        let id = row.id.clone();
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        self.rebuild();
    }

    /// `h`: cierra la fila seleccionada o, si ya está cerrada o es una nota,
    /// sube a la que la contiene.
    fn collapse_or_parent(&mut self) {
        let (Some(row), Some(at)) = (self.selected_entry(), self.selected_row) else {
            return;
        };
        if row.is_expanded() {
            self.toggle_selected();
            return;
        }
        let depth = row.depth;
        if let Some(parent) = self.rows[..at].iter().rposition(|row| row.depth < depth) {
            self.select_row(parent);
        }
    }

    /// `l`: abre la fila seleccionada o, si ya está abierta, baja a su
    /// primera hija.
    fn expand_or_enter(&mut self) {
        let (Some(row), Some(at)) = (self.selected_entry(), self.selected_row) else {
            return;
        };
        if row.is_expanded() {
            if self
                .rows
                .get(at + 1)
                .is_some_and(|next| next.depth > row.depth)
            {
                self.select_row(at + 1);
            }
        } else {
            self.toggle_selected();
        }
    }

    fn selected_path(&self) -> Option<&VaultPath> {
        self.selected_entry()?.note()
    }

    fn selected_file(&self) -> Option<&VaultPath> {
        self.selected_path()
    }
}

#[cfg(test)]
mod tests;
