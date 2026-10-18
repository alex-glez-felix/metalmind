//! Renombrar o mover un archivo sin romper los links: el plan de ediciones
//! que deja cada link resolviendo a lo mismo que antes del rename.

pub(super) mod form;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::Range;

use super::names::{Names, Renamed, key_of};
use super::{IndexedLink, LinkId, LinkIndex, LinkTarget};
use crate::links::edit::{FileEdits, TextEdit};
use crate::path::VaultPath;

/// Las ediciones que mantienen los links al renombrar `from` a `to`. Se
/// aplican a los archivos en sus rutas de antes y después se renombra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePlan {
    pub from: VaultPath,
    pub to: VaultPath,
    /// Por ruta, cada una con sus ediciones en orden.
    pub files: Vec<FileEdits>,
    /// Links que dejarían de apuntar a su destino y no se pueden reescribir
    /// con su forma (un `#` o `|` en el nombre nuevo de un wikilink).
    pub conflicts: Vec<RenameConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameConflict {
    pub source: VaultPath,
    pub span: Range<usize>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenamePlanError {
    #[error("{0} ya existe")]
    Exists(VaultPath),
    #[error("no se puede mover {from} dentro de sí mismo ({to})")]
    IntoItself { from: VaultPath, to: VaultPath },
}

impl LinkIndex {
    /// El plan para renombrar o mover `from` (un archivo o una carpeta) a
    /// `to`. Solo cambia los links que dejarían de resolver a lo mismo, y
    /// cada uno conserva su forma: con o sin `.md`, nombre, ruta parcial,
    /// desde la raíz o relativa, y sus escapes si es markdown.
    ///
    /// # Errors
    /// Si `to` ya existe o está dentro de `from`.
    pub fn rename_plan(
        &self,
        from: &VaultPath,
        to: &VaultPath,
    ) -> Result<RenamePlan, RenamePlanError> {
        if to.starts_with(from) {
            return Err(RenamePlanError::IntoItself {
                from: from.clone(),
                to: to.clone(),
            });
        }
        if !self.names.within(to).is_empty() {
            return Err(RenamePlanError::Exists(to.clone()));
        }
        let moved = self.names.within(from);
        let after = After::new(&self.names, from, to, &moved);
        let mut files: BTreeMap<VaultPath, Vec<TextEdit>> = BTreeMap::new();
        let mut conflicts = Vec::new();
        for id in self.links_to_check(&moved, &after) {
            let Some(found) = self.link(&id) else {
                continue;
            };
            let LinkTarget::Resolved(target) = &found.link.target else {
                continue;
            };
            let source = after.moved(found.source);
            let target = after.moved(target);
            match after.rewrite(found.link, &source, &target) {
                Rewrite::Keep => {}
                Rewrite::To(new) => files.entry(id.source).or_default().push(TextEdit {
                    range: found.link.target_span.clone(),
                    old: found.link.written_target().to_owned(),
                    new,
                    line: found.link.line,
                }),
                Rewrite::Impossible => conflicts.push(RenameConflict {
                    source: id.source,
                    span: found.link.span.clone(),
                    line: found.link.line,
                }),
            }
        }
        Ok(RenamePlan {
            from: from.clone(),
            to: to.clone(),
            files: files
                .into_iter()
                .map(|(path, edits)| FileEdits { path, edits })
                .collect(),
            conflicts,
        })
    }

    /// Los links que el rename puede cambiar: los que llegan a lo movido,
    /// los de las notas movidas (sus relativos y el desempate por carpeta) y
    /// los que tienen la clave de un nombre nuevo, que podría capturarlos.
    fn links_to_check(&self, moved: &[VaultPath], after: &After<'_>) -> BTreeSet<LinkId> {
        let mut ids: BTreeSet<LinkId> = moved
            .iter()
            .filter_map(|path| self.incoming.get(path))
            .flatten()
            .cloned()
            .collect();
        let added: HashSet<&String> = after.names.added_keys().collect();
        let sources: BTreeSet<&VaultPath> = added
            .iter()
            .filter_map(|key| self.by_key.get(*key))
            .flatten()
            .collect();
        for source in sources {
            let captured = self
                .outgoing(source)
                .iter()
                .enumerate()
                .filter(|(_, link)| {
                    key_of(source, &link.target_text).is_some_and(|key| added.contains(&key))
                });
            ids.extend(captured.map(|(ordinal, _)| LinkId {
                source: source.clone(),
                ordinal,
            }));
        }
        for source in moved {
            ids.extend((0..self.outgoing(source).len()).map(|ordinal| LinkId {
                source: source.clone(),
                ordinal,
            }));
        }
        ids
    }
}

enum Rewrite {
    Keep,
    To(String),
    Impossible,
}

/// El vault como queda tras el rename, para resolver contra él.
struct After<'a> {
    names: Renamed<'a>,
    from: &'a VaultPath,
    to: &'a VaultPath,
}

impl<'a> After<'a> {
    fn new(names: &'a Names, from: &'a VaultPath, to: &'a VaultPath, moved: &[VaultPath]) -> Self {
        Self {
            names: names.renamed(moved, from, to),
            from,
            to,
        }
    }

    fn moved(&self, path: &VaultPath) -> VaultPath {
        path.rebase(self.from, self.to)
            .unwrap_or_else(|| path.clone())
    }

    fn resolve(&self, source: &VaultPath, target: &str) -> LinkTarget {
        self.names.resolve(source, target)
    }

    /// Cómo escribir `link`, desde la nota que tras el rename es `source`,
    /// para que siga resolviendo a `target`.
    fn rewrite(&self, link: &IndexedLink, source: &VaultPath, target: &VaultPath) -> Rewrite {
        let resolves =
            |text: &str| self.resolve(source, text) == LinkTarget::Resolved(target.clone());
        if resolves(&link.target_text) {
            return Rewrite::Keep;
        }
        form::candidates(&link.target_text, source, target)
            .into_iter()
            .filter(|candidate| resolves(candidate))
            .find_map(|candidate| form::write(link, &candidate))
            .map_or(Rewrite::Impossible, Rewrite::To)
    }
}
