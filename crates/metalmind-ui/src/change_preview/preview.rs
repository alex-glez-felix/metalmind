//! El modelo de la vista previa: una fila por edición, archivo nuevo o
//! renombre del plan, cada una se puede quitar, y lo que queda es lo que se
//! aplica.

use std::collections::{BTreeSet, HashSet};

use metalmind_vault::{Changes, FileEdits, TextEdit, VaultPath};

pub struct ChangePreview {
    title: String,
    changes: Changes,
    notices: Vec<Notice>,
    rows: Vec<Change>,
    dropped: BTreeSet<usize>,
    selected: usize,
    /// Los archivos del plan con cambios sin guardar en un buffer.
    unsaved: BTreeSet<VaultPath>,
    /// Los renombres son lo que se pidió: no se pueden quitar.
    fixed_renames: bool,
}

/// Algo que dice el plan sobre sí mismo, como que dos tags se fusionan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    /// Si impide aplicar.
    pub blocks: bool,
}

/// Dónde está, en el plan, lo que muestra una fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Change {
    Edit { file: usize, edit: usize },
    Create(usize),
    Rename(usize),
}

/// Una fila como se pinta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewRow<'a> {
    pub path: &'a VaultPath,
    /// Desde 1, la primera que cambia; `None` en un archivo nuevo o un
    /// renombre.
    pub line: Option<usize>,
    /// Solo las líneas que cambian, sin el salto final: puede tener varias.
    pub before: &'a str,
    pub after: &'a str,
    pub dropped: bool,
    /// No se puede quitar: un renombre fijo.
    pub fixed: bool,
    /// El archivo con cambios sin guardar que pisaría: el que edita, o el
    /// origen o el destino de un renombre.
    pub unsaved: Option<&'a VaultPath>,
}

/// Por qué no se puede aplicar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocked<'a> {
    /// No quedó ningún cambio.
    Nothing,
    Notice(&'a Notice),
    /// Archivos con cambios sin guardar: escribirlos pisaría el buffer.
    Unsaved(Vec<&'a VaultPath>),
}

impl ChangePreview {
    /// `unsaved` dice si un archivo del vault tiene cambios sin guardar.
    /// `None` si no hay nada que mostrar: ni cambios ni avisos. Un aviso que
    /// bloquea se muestra aunque no haya cambios.
    #[must_use]
    pub fn new(
        title: String,
        changes: Changes,
        notices: Vec<Notice>,
        unsaved: impl Fn(&VaultPath) -> bool,
    ) -> Option<Self> {
        if changes.is_empty() && notices.is_empty() {
            return None;
        }
        let edits = changes.edits.iter().enumerate().flat_map(|(file, edits)| {
            (0..edits.edits.len()).map(move |edit| Change::Edit { file, edit })
        });
        let rows: Vec<Change> = edits
            .chain((0..changes.creates.len()).map(Change::Create))
            .chain((0..changes.renames.len()).map(Change::Rename))
            .collect();
        let mut preview = Self {
            title,
            changes,
            notices,
            rows,
            dropped: BTreeSet::new(),
            selected: 0,
            unsaved: BTreeSet::new(),
            fixed_renames: false,
        };
        preview.set_unsaved(unsaved);
        Some(preview)
    }

    /// Con los renombres primero, elegidos y fijos: son lo que se pidió, su
    /// destino es lo que se confirma y tiene que leerse entero en el
    /// detalle, y quitar uno dejaría las ediciones que lo acompañan
    /// apuntando a la ruta vieja.
    #[must_use]
    pub fn with_fixed_renames(mut self) -> Self {
        self.fixed_renames = true;
        self.rows
            .sort_by_key(|change| !matches!(change, Change::Rename(_)));
        self.selected = 0;
        self
    }

    /// Esta vista previa, armada de nuevo, deja quitadas las filas que se
    /// habían quitado en `old` (la misma ruta, lo de antes y lo nuevo) y la
    /// elegida donde estaba.
    pub fn carry_over(&mut self, old: &Self) {
        let key = |preview: &Self, row: usize| {
            preview.row(row).map(|row| {
                (
                    row.path.clone(),
                    row.before.to_owned(),
                    row.after.to_owned(),
                )
            })
        };
        let dropped: HashSet<_> = old
            .dropped
            .iter()
            .filter_map(|&row| key(old, row))
            .collect();
        self.dropped = (0..self.rows.len())
            .filter(|&row| {
                !self.is_fixed(row) && key(self, row).is_some_and(|key| dropped.contains(&key))
            })
            .collect();
        self.selected = old.selected.min(self.rows.len().saturating_sub(1));
    }

    /// Vuelve a mirar qué archivos del plan tienen cambios sin guardar, como
    /// tras un autoguardado con la vista previa abierta.
    pub fn set_unsaved(&mut self, unsaved: impl Fn(&VaultPath) -> bool) {
        let changes = &self.changes;
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
    pub fn notices(&self) -> &[Notice] {
        &self.notices
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
        let changes = &self.changes;
        let unsaved = |path: &VaultPath| self.unsaved.get(path);
        let (path, line, before, after, unsaved) = match *self.rows.get(row)? {
            Change::Edit { file, edit } => {
                let file = changes.edits.get(file)?;
                let edit = file.edits.get(edit)?;
                let (before, after, skipped) = changed_lines(&edit.old, &edit.new);
                let line = Some(edit.line + skipped + 1);
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
            fixed: self.is_fixed(row),
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
    /// quitada. Un renombre fijo no se quita.
    pub fn toggle_selected(&mut self) {
        let row = self.selected;
        if row < self.rows.len() && !self.is_fixed(row) && !self.dropped.remove(&row) {
            self.dropped.insert(row);
        }
    }

    fn is_fixed(&self, row: usize) -> bool {
        self.fixed_renames && matches!(self.rows.get(row), Some(Change::Rename(_)))
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
        if let Some(notice) = self.notices.iter().find(|notice| notice.blocks) {
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

    /// Lo que se aplica: el plan sin las filas quitadas, en su orden.
    #[must_use]
    pub fn changes(&self) -> Changes {
        let plan = &self.changes;
        let dropped: HashSet<Change> = self.dropped.iter().map(|&row| self.rows[row]).collect();
        let keep = |change: Change| !dropped.contains(&change);
        let edits = plan
            .edits
            .iter()
            .enumerate()
            .filter_map(|(file_at, file)| {
                let edits: Vec<TextEdit> = file
                    .edits
                    .iter()
                    .enumerate()
                    .filter(|&(edit, _)| {
                        keep(Change::Edit {
                            file: file_at,
                            edit,
                        })
                    })
                    .map(|(_, edit)| edit.clone())
                    .collect();
                (!edits.is_empty()).then(|| FileEdits {
                    path: file.path.clone(),
                    edits,
                    base_len: file.base_len,
                })
            })
            .collect();
        Changes {
            edits,
            creates: kept(&plan.creates, Change::Create, keep),
            renames: kept(&plan.renames, Change::Rename, keep),
        }
    }
}

/// Los de `items` que no se quitaron; `change` dice qué fila es cada uno.
fn kept<T: Clone>(
    items: &[T],
    change: fn(usize) -> Change,
    keep: impl Fn(Change) -> bool,
) -> Vec<T> {
    items
        .iter()
        .enumerate()
        .filter(|&(at, _)| keep(change(at)))
        .map(|(_, item)| item.clone())
        .collect()
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

/// Lo que cambia de `before` a `after` sin las líneas iguales del principio
/// ni del final, y cuántas se saltaron al principio. Agregar un campo antes
/// del `---` de cierre (que la edición reemplaza para comprobarlo) se ve como
/// la línea nueva.
fn changed_lines<'a>(before: &'a str, after: &'a str) -> (&'a str, &'a str, usize) {
    let lines = |text: &'a str| text.split_inclusive('\n').collect::<Vec<_>>();
    let (old, new) = (lines(before), lines(after));
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let middle = |text: &'a str, lines: &[&str]| {
        let start: usize = lines[..prefix].iter().map(|line| line.len()).sum();
        let end: usize = lines[lines.len() - suffix..]
            .iter()
            .map(|line| line.len())
            .sum();
        text[start..text.len() - end].trim_end_matches(['\n', '\r'])
    };
    (middle(before, &old), middle(after, &new), prefix)
}
