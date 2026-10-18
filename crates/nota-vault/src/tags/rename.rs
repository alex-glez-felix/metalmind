//! Armar los planes de 05 §5: renombrar un tag o un alias, fusionar y pasar
//! un alias al nombre principal.

use crate::changes::{Rename, TextEdit};
use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::key::{TagKey, tag_name};
use crate::tags::note::{FrontmatterSpot, TagOccurrence, TagSource};
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
    /// pasan a la forma establecida de `into`.
    ///
    /// # Errors
    /// Si algún nombre no es válido o alguno de los dos no existe.
    pub fn plan_merge(&self, from: &str, into: &str) -> Result<TagPlan, PlanError> {
        let from = self.known(from)?;
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
            (Some(page), _) => plan
                .changes
                .renames
                .extend(self.page_rename(&page.path, to)),
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
        if &rewrite.key != alias && &rewrite.key != owner {
            let taken = match self.graph.alias_of(&rewrite.key) {
                Some(other) if other != owner => Some(other.clone()),
                _ => self.graph.page(&rewrite.key).map(|page| page.key.clone()),
            };
            match taken {
                Some(by) => plan.notices.push(PlanNotice::AliasTaken {
                    alias: rewrite.key.clone(),
                    by,
                }),
                None if self.index.tag(&rewrite.key).is_some()
                    && self.graph.alias_of(&rewrite.key).is_none() =>
                {
                    plan.notices.push(PlanNotice::AliasAbsorbs {
                        tag: rewrite.key.clone(),
                        of: owner.clone(),
                    });
                }
                None => {}
            }
        }
        let mut edits = Edits::default();
        self.rewrite_notes(&rewrite, &mut edits);
        self.rewrite_pages(&rewrite, &mut edits);
        plan.changes.edits = edits.into_files();
        plan
    }

    fn page_rename(&self, from: &VaultPath, name: &str) -> Option<Rename> {
        let to = self.folder.join(&format!("{name}.md")).ok()?;
        (&to != from).then(|| Rename {
            from: from.clone(),
            to,
        })
    }

    /// Las ocurrencias en notas y en el cuerpo y `tags:` de las páginas. Un
    /// valor de `tags:` que queda repetido se quita de la lista.
    fn rewrite_notes(&self, rewrite: &Rewrite<'_>, edits: &mut Edits) {
        for (path, note) in self.index.all() {
            let tags = &note.tags;
            if !tags.occurrences.iter().any(|tag| &tag.key == rewrite.from) {
                continue;
            }
            let mut seen = false;
            for occurrence in &tags.occurrences {
                if let Some(edit) = rewrite.edit(occurrence, &mut seen) {
                    edits.push(path, tags.hash, edit);
                }
            }
        }
    }

    /// Los `aliases` y `parents` de las páginas que nombran la clave. Un
    /// padre que queda repetido o que sería la página misma se quita.
    fn rewrite_pages(&self, rewrite: &Rewrite<'_>, edits: &mut Edits) {
        for page in self.graph.pages() {
            let Some(hash) = self.index.note(&page.path).map(|note| note.hash) else {
                continue;
            };
            for alias in page
                .aliases
                .iter()
                .filter(|alias| &alias.key == rewrite.from)
            {
                edits.push(&page.path, hash, rewrite.replace(alias));
            }
            let mut seen = page.key == rewrite.key;
            for parent in &page.parents {
                if let Some(edit) = rewrite.edit(parent, &mut seen) {
                    edits.push(&page.path, hash, edit);
                }
            }
        }
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
    /// quitarla si repite la clave nueva en su lista, o nada. `seen` dice si
    /// la lista ya tiene la clave nueva.
    fn edit(&self, occurrence: &TagOccurrence, seen: &mut bool) -> Option<TextEdit> {
        let renamed = &occurrence.key == self.from;
        let lands = renamed || occurrence.key == self.key;
        match &occurrence.source {
            TagSource::Frontmatter(FrontmatterSpot { removal, .. }) if lands && *seen => {
                Some(TextEdit {
                    range: removal.clone(),
                    text: String::new(),
                })
            }
            TagSource::Frontmatter(_) => {
                *seen |= lands;
                renamed.then(|| self.replace(occurrence))
            }
            TagSource::Inline => renamed.then(|| self.replace(occurrence)),
        }
    }

    fn replace(&self, occurrence: &TagOccurrence) -> TextEdit {
        let literal = match &occurrence.source {
            TagSource::Inline => true,
            TagSource::Frontmatter(spot) => spot.literal,
        };
        // Un nombre de tag nunca lleva `"` ni `\`: entre comillas siempre es él mismo.
        let text = if literal {
            self.name.to_owned()
        } else {
            format!("\"{}\"", self.name)
        };
        TextEdit {
            range: occurrence.range.clone(),
            text,
        }
    }
}

fn valid_name(text: &str) -> Result<&str, PlanError> {
    tag_name(text).ok_or_else(|| PlanError::InvalidName(text.to_owned()))
}
