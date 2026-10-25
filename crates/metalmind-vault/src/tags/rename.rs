//! Armar los planes de 05 §5: renombrar un tag o un alias, fusionar y pasar
//! un alias al nombre principal.

use crate::changes::Rename;
use crate::edit::TextEdit;
use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::key::{TagKey, tag_name};
use crate::tags::metadata::not_portable;
use crate::tags::note::{TagOccurrence, TagSource};
use crate::tags::refactor::{Edits, PlanError, PlanNotice, TagPlan};

/// Qué ocurrencias reescribe un plan y cómo se escriben después.
struct Rewrite<'n> {
    /// La clave que se reescribe.
    from: &'n TagKey,
    /// Cómo queda escrita, y su clave.
    name: &'n str,
    key: TagKey,
}

impl Tags {
    /// Renombrar `from` a `to` en todo el vault (`:tagrename`). Si `from` es
    /// un alias, renombra el alias; si `to` ya existe, los fusiona.
    ///
    /// # Errors
    /// Si algún nombre no es válido o `from` no existe.
    pub fn plan_rename(&self, from: &str, to: &str) -> Result<TagPlan, PlanError> {
        let from = self.known(from)?;
        let to = valid_name(to)?;
        match self.graph.alias_of(&from) {
            Some(owner) => Ok(self.rename_alias(&from, owner, to)),
            None => Ok(self.rename_tag(&from, to)),
        }
    }

    /// Fusionar `from` en `into` (`:tagmerge`): las ocurrencias de `from`
    /// pasan a la forma establecida de `into`. Un alias vale por su tag, como
    /// en una consulta: fusionar `#rs` fusiona `#rust`.
    ///
    /// # Errors
    /// Si algún nombre no es válido o alguno de los dos no existe.
    pub fn plan_merge(&self, from: &str, into: &str) -> Result<TagPlan, PlanError> {
        let from = self.known(from)?;
        let from = self.graph.resolve(&from).clone();
        let into = self.known(into)?;
        let into = self.graph.resolve(&into);
        let name = self.display_name(into).unwrap_or(into.as_str());
        Ok(self.rename_tag(&from, name))
    }

    /// Reescribir las ocurrencias de un alias con el nombre de su tag
    /// (`:tagmain`). El alias se queda en la página.
    ///
    /// # Errors
    /// Si `alias` no es un alias que aplique.
    pub fn plan_use_main_name(&self, alias: &str) -> Result<TagPlan, PlanError> {
        let key = self.known(alias)?;
        let owner = self
            .graph
            .alias_of(&key)
            .ok_or_else(|| PlanError::NotAnAlias(alias.to_owned()))?;
        let name = self.display_name(owner).unwrap_or(owner.as_str());
        let mut edits = Edits::default();
        self.rewrite_notes(&Rewrite::new(&key, name), &mut edits);
        let mut plan = TagPlan::default();
        plan.changes.edits = edits.into_files();
        Ok(plan)
    }

    fn known(&self, text: &str) -> Result<TagKey, PlanError> {
        let key = TagKey::new(valid_name(text)?);
        if self.exists(&key) {
            Ok(key)
        } else {
            Err(PlanError::UnknownTag(text.to_owned()))
        }
    }

    fn rename_tag(&self, from: &TagKey, to: &str) -> TagPlan {
        let rewrite = Rewrite::new(from, to);
        let mut plan = TagPlan::default();
        let target = self.graph.resolve(&rewrite.key).clone();
        if &target != from && self.exists(&rewrite.key) {
            plan.notices.push(PlanNotice::Merges {
                into: target.clone(),
            });
        }
        let from_page = self.graph.page(from);
        match (from_page, self.graph.page(&target)) {
            (Some(page), Some(other)) if &target != from => {
                plan.notices.push(PlanNotice::BothHavePages {
                    from: page.path.clone(),
                    into: other.path.clone(),
                });
            }
            (Some(page), _) => {
                if let Some(rename) = self.page_rename(&page.path, to) {
                    plan.notices.extend(not_portable(&rewrite.key, &rename.to));
                    plan.changes.renames.push(rename);
                }
            }
            (None, _) => {}
        }
        let mut edits = Edits::default();
        self.rewrite_notes(&rewrite, &mut edits);
        self.rewrite_pages(&rewrite, &mut edits);
        plan.changes.edits = edits.into_files();
        plan
    }

    fn rename_alias(&self, alias: &TagKey, owner: &TagKey, to: &str) -> TagPlan {
        let rewrite = Rewrite::new(alias, to);
        let mut plan = TagPlan {
            notices: vec![PlanNotice::RenamesAlias { of: owner.clone() }],
            ..TagPlan::default()
        };
        if &rewrite.key != alias {
            plan.notices.extend(self.alias_notice(&rewrite.key, owner));
        }
        let mut edits = Edits::default();
        self.rewrite_notes(&rewrite, &mut edits);
        self.rewrite_pages(&rewrite, &mut edits);
        plan.changes.edits = edits.into_files();
        plan
    }

    /// Lo que la vista previa avisa si `alias` pasa a ser alias de `owner`:
    /// bloquea si es una página u otra página ya lo declara (aunque ahí no
    /// aplique por un choque), y avisa si las notas ya lo escriben como tag.
    pub(crate) fn alias_notice(&self, alias: &TagKey, owner: &TagKey) -> Option<PlanNotice> {
        if alias == owner {
            return None;
        }
        let taken = self.graph.page(alias).or_else(|| {
            self.graph.pages().find(|page| {
                &page.key != owner && page.aliases.iter().any(|declared| &declared.key == alias)
            })
        });
        if let Some(page) = taken.filter(|page| &page.key != owner) {
            return Some(PlanNotice::AliasTaken {
                alias: alias.clone(),
                by: page.key.clone(),
            });
        }
        (self.index.tag(alias).is_some() && self.graph.alias_of(alias).is_none()).then(|| {
            PlanNotice::AliasAbsorbs {
                tag: alias.clone(),
                of: owner.clone(),
            }
        })
    }

    fn page_rename(&self, from: &VaultPath, name: &str) -> Option<Rename> {
        let to = self.page_file(name)?;
        (&to != from).then(|| Rename {
            from: from.clone(),
            to,
        })
    }

    /// Las ocurrencias en notas y en el cuerpo y `tags:` de las páginas. Un
    /// valor de `tags:` que queda repetido se quita de la lista.
    fn rewrite_notes(&self, rewrite: &Rewrite<'_>, edits: &mut Edits) {
        let notes = self
            .index
            .tag(rewrite.from)
            .into_iter()
            .flat_map(|entry| &entry.notes);
        // Las páginas no cuentan en el índice por tag: se miran todas.
        let pages = self.pages.keys().filter(|path| self.is_page(path));
        let indexed = notes
            .chain(pages)
            .filter_map(|path| Some((path, self.index.note(path)?)));
        for (path, tags) in indexed {
            if !tags.occurrences.iter().any(|tag| &tag.key == rewrite.from) {
                continue;
            }
            let mut list = List::new(false);
            for occurrence in &tags.occurrences {
                if let Some(edit) = rewrite.edit(occurrence, &mut list) {
                    edits.push(path, edit);
                }
            }
        }
    }

    /// Los `aliases` y `parents` de las páginas que nombran la clave. Un
    /// padre que queda repetido o que sería la página misma (con su nombre
    /// nuevo, si es la que se renombra) se quita.
    fn rewrite_pages(&self, rewrite: &Rewrite<'_>, edits: &mut Edits) {
        for page in self.graph.pages() {
            for alias in page
                .aliases
                .iter()
                .filter(|alias| &alias.key == rewrite.from)
            {
                edits.push(&page.path, rewrite.replace(alias));
            }
            let renamed = if &page.key == rewrite.from {
                &rewrite.key
            } else {
                &page.key
            };
            let mut list = List::new(renamed == &rewrite.key);
            for parent in &page.parents {
                if let Some(edit) = rewrite.edit(parent, &mut list) {
                    edits.push(&page.path, edit);
                }
            }
        }
    }
}

/// Lo recorrido de una lista del frontmatter.
struct List {
    /// Ya tiene la clave nueva: otra igual sobra.
    seen: bool,
    /// Todos los valores de antes se quitan.
    prefix: bool,
}

impl List {
    fn new(seen: bool) -> Self {
        Self { seen, prefix: true }
    }
}

impl<'n> Rewrite<'n> {
    fn new(from: &'n TagKey, name: &'n str) -> Self {
        Self {
            from,
            name,
            key: TagKey::new(name),
        }
    }

    /// Qué hacer con una ocurrencia de una lista (o del texto): reescribirla,
    /// quitarla si repite la clave nueva en su lista, o nada.
    fn edit(&self, occurrence: &TagOccurrence, list: &mut List) -> Option<TextEdit> {
        let renamed = &occurrence.key == self.from;
        let lands = renamed || occurrence.key == self.key;
        match &occurrence.source {
            TagSource::Frontmatter(spot) if lands && list.seen => {
                let removal = if list.prefix {
                    &spot.removal_first
                } else {
                    &spot.removal
                };
                Some(TextEdit {
                    range: removal.range.clone(),
                    old: removal.text.clone(),
                    new: String::new(),
                    line: occurrence.line,
                })
            }
            TagSource::Frontmatter(_) => {
                list.seen |= lands;
                list.prefix = false;
                renamed.then(|| self.replace(occurrence))
            }
            TagSource::Inline => renamed.then(|| self.replace(occurrence)),
        }
    }

    fn replace(&self, occurrence: &TagOccurrence) -> TextEdit {
        let escaped = matches!(
            &occurrence.source,
            TagSource::Frontmatter(spot) if spot.escaped.is_some()
        );
        // Un nombre de tag nunca lleva `"` ni `\`: entre comillas siempre es él mismo.
        let new = if escaped {
            format!("\"{}\"", self.name)
        } else {
            self.name.to_owned()
        };
        TextEdit {
            range: occurrence.range.clone(),
            old: occurrence.written().to_owned(),
            new,
            line: occurrence.line,
        }
    }
}

fn valid_name(text: &str) -> Result<&str, PlanError> {
    tag_name(text).ok_or_else(|| PlanError::InvalidName(text.to_owned()))
}
