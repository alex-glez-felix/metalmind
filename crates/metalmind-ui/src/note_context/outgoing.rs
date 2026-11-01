//! La pestaña de salientes del panel de contexto, sin GPUI: a qué notas y
//! adjuntos llevan los links de la nota activa, una fila por destino en el
//! orden en que aparecen, y al final los que no llevan a un solo archivo.

use std::sync::Arc;

use metalmind_vault::{IndexedLink, LinkIndex, LinkTarget, VaultChange, VaultPath};

use crate::panel::Outline;
use crate::workspace::LinePosition;

/// A dónde llevan los links de una fila.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Destination {
    /// Una nota, o un adjunto como el de `![[foto.png]]`.
    File(VaultPath),
    /// Varias notas empatan con lo escrito: `gd` sobre el link deja elegir.
    Ambiguous(String),
    /// Ninguna nota se llama así.
    Missing(String),
}

/// Un destino de los links de la nota activa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub destination: Destination,
    /// Cuántos links de la nota activa llevan ahí.
    pub count: usize,
    /// Dónde está el primero en la nota activa.
    pub first: LinePosition,
}

#[derive(Clone, Debug)]
pub struct OutgoingOutline {
    index: Arc<LinkIndex>,
    note: Option<VaultPath>,
    selected: Option<Destination>,
    rows: Vec<Row>,
    selected_row: Option<usize>,
    stale: bool,
}

impl OutgoingOutline {
    #[must_use]
    pub fn new(index: Arc<LinkIndex>) -> Self {
        Self {
            index,
            note: None,
            selected: None,
            rows: Vec::new(),
            selected_row: None,
            stale: true,
        }
    }

    /// Otra nota empieza en su primera fila.
    pub fn show_note(&mut self, note: Option<VaultPath>) {
        if self.note != note {
            self.note = note;
            self.selected = None;
            self.selected_row = None;
            self.stale = true;
        }
    }

    pub fn set_index(&mut self, index: Arc<LinkIndex>) {
        if !Arc::ptr_eq(&self.index, &index) {
            self.index = index;
            self.stale = true;
        }
    }

    /// Tras un lote del watcher: la nota activa y el destino elegido siguen
    /// a sus renombres.
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
            if let Some(Destination::File(path)) = &mut self.selected {
                moved(path);
            }
        }
    }

    /// Rehace las filas si cambió algo; la elegida se queda en su destino si
    /// sigue, y si no en la fila que ocupaba.
    pub fn refresh(&mut self) {
        if !std::mem::take(&mut self.stale) {
            return;
        }
        let previous_row = self.selected_row;
        self.rows = self
            .note
            .as_ref()
            .map(|note| rows(self.index.outgoing(note)))
            .unwrap_or_default();
        self.selected_row = self
            .selected
            .as_ref()
            .and_then(|chosen| self.rows.iter().position(|row| row.destination == *chosen));
        match (self.selected_row, previous_row) {
            (Some(row), _) | (None, Some(row)) => self.select_row(row),
            (None, None) => self.select_row(0),
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

    /// Dónde abre la fila elegida: un archivo, donde quedó el cursor la
    /// última vez; lo que no lleva a uno, en el link.
    #[must_use]
    pub fn selected_position(&self) -> Option<LinePosition> {
        let row = self.selected_entry()?;
        match row.destination {
            Destination::File(_) => None,
            Destination::Ambiguous(_) | Destination::Missing(_) => Some(row.first),
        }
    }
}

/// Una fila por destino, en el orden de su primer link; los archivos antes
/// que lo que no lleva a uno solo, que es lo que hay que arreglar.
fn rows(links: &[IndexedLink]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for link in links {
        let Some(destination) = destination(link) else {
            continue;
        };
        match rows.iter_mut().find(|row| row.destination == destination) {
            Some(row) => row.count += 1,
            None => rows.push(Row {
                destination,
                count: 1,
                first: LinePosition {
                    line: link.line,
                    column: link.column,
                },
            }),
        }
    }
    rows.sort_by_key(|row| !matches!(row.destination, Destination::File(_)));
    rows
}

/// `[[#Heading]]` no sale de la nota: no es un destino.
fn destination(link: &IndexedLink) -> Option<Destination> {
    match &link.target {
        LinkTarget::SameNote => None,
        LinkTarget::Resolved(path) => Some(Destination::File(path.clone())),
        LinkTarget::Ambiguous(_) => Some(Destination::Ambiguous(link.target_text.clone())),
        LinkTarget::Unresolved => Some(Destination::Missing(link.target_text.clone())),
    }
}

impl Outline for OutgoingOutline {
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
        self.selected = Some(self.rows[row].destination.clone());
        self.selected_row = Some(row);
    }

    fn collapse_or_parent(&mut self) {}

    fn expand_or_enter(&mut self) {}

    fn toggle_selected(&mut self) {}

    fn selected_path(&self) -> Option<&VaultPath> {
        self.selected_file()
    }

    /// Un archivo se abre; lo que no lleva a uno solo abre la nota activa en
    /// el link, para arreglarlo o elegir con `gd`.
    fn selected_file(&self) -> Option<&VaultPath> {
        match &self.selected_entry()?.destination {
            Destination::File(path) => Some(path),
            Destination::Ambiguous(_) | Destination::Missing(_) => self.note.as_ref(),
        }
    }
}

#[cfg(test)]
#[path = "outgoing_tests.rs"]
mod tests;
