//! La vista previa de un cambio de tags: una fila por edición, archivo
//! nuevo o renombre del plan, cada una se puede quitar, y lo que queda es
//! lo que se aplica.

use std::collections::BTreeSet;

use metalmind_vault::tags::{PlanNotice, TagPlan};
use metalmind_vault::{Changes, FileEdits, TextEdit, VaultPath};

pub struct TagChangePreview {
    title: String,
    plan: TagPlan,
    rows: Vec<Change>,
    dropped: BTreeSet<usize>,
    selected: usize,
    /// Los archivos del plan con cambios sin guardar en un buffer.
    unsaved: BTreeSet<VaultPath>,
}

/// Dónde está, en el plan, lo que muestra una fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Edit { file: usize, edit: usize },
    Create(usize),
    Rename(usize),
}

/// Una fila como se pinta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewRow<'a> {
    pub path: &'a VaultPath,
    /// Desde 1; `None` en un archivo nuevo o un renombre.
    pub line: Option<usize>,
    pub before: &'a str,
    pub after: &'a str,
    pub dropped: bool,
    /// El archivo con cambios sin guardar que pisaría: el que edita, o el
    /// origen o el destino de un renombre.
    pub unsaved: Option<&'a VaultPath>,
}

/// Por qué no se puede aplicar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocked<'a> {
    /// No quedó ningún cambio.
    Nothing,
    Notice(&'a PlanNotice),
    /// Archivos con cambios sin guardar: escribirlos pisaría el buffer.
    Unsaved(Vec<&'a VaultPath>),
}

impl TagChangePreview {
    /// `unsaved` dice si un archivo del vault tiene cambios sin guardar.
    /// `None` si el plan no tiene nada que mostrar: ni cambios ni avisos.
    /// Un aviso que bloquea se muestra aunque no haya cambios.
    #[must_use]
    pub fn new(title: String, plan: TagPlan, unsaved: impl Fn(&VaultPath) -> bool) -> Option<Self> {
        if plan.changes.is_empty() && plan.notices.is_empty() {
            return None;
        }
        let changes = &plan.changes;
        let edits = changes.edits.iter().enumerate().flat_map(|(file, edits)| {
            (0..edits.edits.len()).map(move |edit| Change::Edit { file, edit })
        });
        let rows: Vec<Change> = edits
            .chain((0..changes.creates.len()).map(Change::Create))
            .chain((0..changes.renames.len()).map(Change::Rename))
            .collect();
        let mut preview = Self {
            title,
            plan,
            rows,
            dropped: BTreeSet::new(),
            selected: 0,
            unsaved: BTreeSet::new(),
        };
        preview.set_unsaved(unsaved);
        Some(preview)
    }

    /// Vuelve a mirar qué archivos del plan tienen cambios sin guardar, como
    /// tras un autoguardado con la vista previa abierta.
    pub fn set_unsaved(&mut self, unsaved: impl Fn(&VaultPath) -> bool) {
        let changes = &self.plan.changes;
        let renames = changes.renames.iter();
        self.unsaved = changes
            .edits
            .iter()
            .map(|file| &file.path)
            .chain(renames.flat_map(|rename| [&rename.from, &rename.to]))
            .filter(|path| unsaved(path))
            .cloned()
            .collect();
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn notices(&self) -> &[PlanNotice] {
        &self.plan.notices
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// La fila `row`, si existe.
    #[must_use]
    pub fn row(&self, row: usize) -> Option<PreviewRow<'_>> {
        let changes = &self.plan.changes;
        let unsaved = |path: &VaultPath| self.unsaved.get(path);
        let (path, line, before, after, unsaved) = match *self.rows.get(row)? {
            Change::Edit { file, edit } => {
                let file = changes.edits.get(file)?;
                let edit = file.edits.get(edit)?;
                let line = Some(edit.line + 1);
                let (before, after) = (edit.old.as_str(), edit.new.as_str());
                (&file.path, line, before, after, unsaved(&file.path))
            }
            Change::Create(index) => {
                let create = changes.creates.get(index)?;
                let path = &create.path;
                (path, None, "", path.as_str(), unsaved(path))
            }
            Change::Rename(index) => {
                let rename = changes.renames.get(index)?;
                let (from, to) = (&rename.from, &rename.to);
                let busy = unsaved(from).or_else(|| unsaved(to));
                (from, None, from.as_str(), to.as_str(), busy)
            }
        };
        Some(PreviewRow {
            path,
            line,
            before,
            after,
            dropped: self.dropped.contains(&row),
            unsaved,
        })
    }

    /// La siguiente, y tras la última la primera.
    pub fn select_next(&mut self) {
        if !self.rows.is_empty() {
            self.selected = (self.selected + 1) % self.rows.len();
        }
    }

    /// La anterior, y antes de la primera la última.
    pub fn select_previous(&mut self) {
        if !self.rows.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.rows.len() - 1);
        }
    }

    pub fn select(&mut self, row: usize) {
        if row < self.rows.len() {
            self.selected = row;
        }
    }

    /// Quita la fila elegida de lo que se aplica, o la devuelve si ya estaba
    /// quitada.
    pub fn toggle_selected(&mut self) {
        if self.selected < self.rows.len() && !self.dropped.remove(&self.selected) {
            self.dropped.insert(self.selected);
        }
    }

    /// Cuántos cambios quedan y en cuántos archivos.
    #[must_use]
    pub fn summary(&self) -> String {
        let kept: Vec<PreviewRow<'_>> = (0..self.rows.len())
            .filter_map(|row| self.row(row))
            .filter(|row| !row.dropped)
            .collect();
        let files: BTreeSet<&VaultPath> = kept.iter().map(|row| row.path).collect();
        let changes = plural(kept.len(), "cambio", "cambios");
        let files = plural(files.len(), "archivo", "archivos");
        format!("{changes} en {files}")
    }

    /// Lo primero que impide aplicar, si algo lo impide.
    #[must_use]
    pub fn blocked(&self) -> Option<Blocked<'_>> {
        if let Some(notice) = self.plan.notices.iter().find(|notice| notice.blocks()) {
            return Some(Blocked::Notice(notice));
        }
        let kept = || {
            (0..self.rows.len())
                .filter_map(|row| self.row(row))
                .filter(|row| !row.dropped)
        };
        let unsaved: BTreeSet<&VaultPath> = kept().filter_map(|row| row.unsaved).collect();
        if !unsaved.is_empty() {
            return Some(Blocked::Unsaved(unsaved.into_iter().collect()));
        }
        kept().next().is_none().then_some(Blocked::Nothing)
    }

    /// Lo que se aplica: el plan sin las filas quitadas. Las filas van en
    /// el orden del plan: ediciones, archivos nuevos y renombres.
    #[must_use]
    pub fn changes(&self) -> Changes {
        let plan = &self.plan.changes;
        let mut kept = (0..self.rows.len()).map(|row| !self.dropped.contains(&row));
        let mut keep = || kept.next().unwrap_or(false);
        let edits = plan
            .edits
            .iter()
            .filter_map(|file| {
                let edits: Vec<TextEdit> = file.edits.iter().filter(|_| keep()).cloned().collect();
                (!edits.is_empty()).then(|| FileEdits {
                    path: file.path.clone(),
                    edits,
                })
            })
            .collect();
        let creates = plan.creates.iter().filter(|_| keep()).cloned().collect();
        let renames = plan.renames.iter().filter(|_| keep()).cloned().collect();
        Changes {
            edits,
            creates,
            renames,
        }
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}
