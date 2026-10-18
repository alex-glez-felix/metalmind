//! Cambiar la metadata de un tag desde una acción (05 §3): color, ícono,
//! alias y padres. Se escribe en el frontmatter de su página tocando solo
//! ese campo, y si el tag no tiene página, nota la crea.

use unicode_normalization::UnicodeNormalization as _;

use crate::changes::{Changes, FileEdit, NewFile, TextEdit};
use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::key::{TagKey, tag_name};
use crate::tags::page::{PageField, PageLayout, TagColor, TagIcon, TagPage};
use crate::tags::refactor::{PlanError, PlanNotice, TagPlan};

/// El valor nuevo de un campo de la página. `None` o una lista vacía quitan
/// el campo; la página se queda aunque no le quede nada (05 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageChange {
    Color(Option<TagColor>),
    Icon(Option<TagIcon>),
    /// Nombres de tag, con o sin `#`.
    Aliases(Vec<String>),
    Parents(Vec<String>),
}

/// Nombres que Windows reserva: un archivo así no se sincroniza allá (05 §1).
const RESERVED_ON_WINDOWS: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

impl Tags {
    /// Dónde está la página de `tag` o dónde se crearía, con el tag ya
    /// resuelto si es un alias.
    ///
    /// # Errors
    /// Si `tag` no es un nombre de tag válido.
    pub fn page_path(&self, tag: &str) -> Result<VaultPath, PlanError> {
        let key = self.tag_key(tag)?;
        match self.graph.page(&key) {
            Some(page) => Ok(page.path.clone()),
            None => self.new_page_path(&key, tag),
        }
    }

    /// Crear la página vacía de `tag` ("Agregar descripción", 05 §7). Si ya
    /// tiene, el plan no hace nada.
    ///
    /// # Errors
    /// Si `tag` no es un nombre de tag válido.
    pub fn plan_create_page(&self, tag: &str) -> Result<TagPlan, PlanError> {
        let key = self.tag_key(tag)?;
        if self.graph.page(&key).is_some() {
            return Ok(TagPlan::default());
        }
        self.create_page(&key, tag, String::new())
    }

    /// Cambiar un campo de la página de `tag` (`tags::set_color`,
    /// `tags::set_icon`, `tags::edit_aliases`, `tags::edit_parents`). Un
    /// alias cambia la página de su tag.
    ///
    /// # Errors
    /// Si un nombre no es válido, un tag se pone a sí mismo de padre o el
    /// frontmatter de la página no se puede editar sin reescribirlo.
    pub fn plan_page_change(&self, tag: &str, change: &PageChange) -> Result<TagPlan, PlanError> {
        let key = self.tag_key(tag)?;
        let (field, value) = self.field_value(&key, change)?;
        let notices = match change {
            PageChange::Aliases(aliases) => self.alias_notices(&key, aliases),
            _ => Vec::new(),
        };
        let mut plan = match self.graph.page(&key) {
            Some(page) => edit_page(self, page, field, value.as_deref())?,
            None => match value {
                Some(value) => {
                    let text = format!("---\n{}: {value}\n---\n", field.key());
                    self.create_page(&key, tag, text)?
                }
                None => TagPlan::default(),
            },
        };
        plan.notices.extend(notices);
        Ok(plan)
    }

    /// La clave de `tag`, resuelta si es un alias.
    fn tag_key(&self, tag: &str) -> Result<TagKey, PlanError> {
        let name = tag_name(tag).ok_or_else(|| PlanError::InvalidName(tag.to_owned()))?;
        Ok(self.graph.resolve(&TagKey::new(name)).clone())
    }

    /// `tags/<forma establecida>.md`, en NFC (05 §1).
    fn new_page_path(&self, key: &TagKey, tag: &str) -> Result<VaultPath, PlanError> {
        let name = self
            .display_name(key)
            .or_else(|| tag_name(tag))
            .unwrap_or(key.as_str());
        let name: String = name.nfc().collect();
        self.folder
            .join(&format!("{name}.md"))
            .map_err(|_| PlanError::InvalidName(name))
    }

    fn create_page(&self, key: &TagKey, tag: &str, text: String) -> Result<TagPlan, PlanError> {
        let path = self.new_page_path(key, tag)?;
        let mut notices = Vec::new();
        if RESERVED_ON_WINDOWS.contains(&key.as_str()) {
            notices.push(PlanNotice::NotPortable { path: path.clone() });
        }
        Ok(TagPlan {
            changes: Changes {
                creates: vec![NewFile { path, text }],
                ..Changes::default()
            },
            notices,
        })
    }

    /// El campo y su valor en YAML, o `None` si se quita.
    fn field_value(
        &self,
        key: &TagKey,
        change: &PageChange,
    ) -> Result<(PageField, Option<String>), PlanError> {
        Ok(match change {
            PageChange::Color(color) => (
                PageField::Color,
                color.as_ref().map(color_yaml).transpose()?,
            ),
            PageChange::Icon(icon) => (PageField::Icon, icon.as_ref().map(icon_yaml).transpose()?),
            PageChange::Aliases(names) => (PageField::Aliases, list_yaml(&names_of(names)?)),
            PageChange::Parents(names) => {
                let names = names_of(names)?;
                if let Some(own) = names
                    .iter()
                    .find(|name| self.graph.resolve(&TagKey::new(name)) == key)
                {
                    return Err(PlanError::OwnParent((*own).to_owned()));
                }
                (PageField::Parents, list_yaml(&names))
            }
        })
    }

    /// Lo que la vista previa avisa de los alias nuevos de `key`.
    fn alias_notices(&self, key: &TagKey, aliases: &[String]) -> Vec<PlanNotice> {
        let mut notices = Vec::new();
        for alias in aliases.iter().filter_map(|alias| TagKey::parse(alias)) {
            if &alias == key {
                continue;
            }
            let taken = match self.graph.alias_of(&alias) {
                Some(owner) if owner != key => Some(owner.clone()),
                Some(_) => None,
                None => self.graph.page(&alias).map(|page| page.key.clone()),
            };
            match taken {
                Some(by) => notices.push(PlanNotice::AliasTaken { alias, by }),
                None if self.index.tag(&alias).is_some()
                    && self.graph.alias_of(&alias).is_none() =>
                {
                    notices.push(PlanNotice::AliasAbsorbs {
                        tag: alias,
                        of: key.clone(),
                    });
                }
                None => {}
            }
        }
        notices
    }
}

impl PageField {
    /// La clave del frontmatter.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Icon => "icon",
            Self::Aliases => "aliases",
            Self::Parents => "parents",
        }
    }
}

/// La edición de un campo en una página que ya existe.
fn edit_page(
    tags: &Tags,
    page: &TagPage,
    field: PageField,
    value: Option<&str>,
) -> Result<TagPlan, PlanError> {
    let Some(hash) = tags.index.note(&page.path).map(|note| note.hash) else {
        return Ok(TagPlan::default());
    };
    let key = field.key();
    let edit = match (&page.layout, value) {
        (PageLayout::Unreadable, _) => return Err(PlanError::UneditablePage(page.path.clone())),
        (PageLayout::Bare { .. }, None) => None,
        (&PageLayout::Bare { at, crlf }, Some(value)) => {
            let eol = eol(crlf);
            Some(TextEdit {
                range: at..at,
                text: format!("---{eol}{key}: {value}{eol}---{eol}"),
            })
        }
        (PageLayout::Block { entries, end, crlf }, value) => {
            let line = value.map(|value| format!("{key}: {value}{}", eol(*crlf)));
            match (entries.iter().find(|entry| entry.key == key), line) {
                (Some(entry), line) => Some(TextEdit {
                    range: entry.span.clone(),
                    text: line.unwrap_or_default(),
                }),
                (None, Some(line)) => Some(TextEdit {
                    range: *end..*end,
                    text: line,
                }),
                (None, None) => None,
            }
        }
    };
    let mut plan = TagPlan::default();
    if let Some(edit) = edit {
        plan.changes.edits.push(FileEdit {
            path: page.path.clone(),
            hash,
            edits: vec![edit],
        });
    }
    Ok(plan)
}

fn eol(crlf: bool) -> &'static str {
    if crlf { "\r\n" } else { "\n" }
}

/// Los nombres sin `#` y sin repetir clave, en orden.
fn names_of(names: &[String]) -> Result<Vec<&str>, PlanError> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for name in names {
        let valid = tag_name(name).ok_or_else(|| PlanError::InvalidName(name.clone()))?;
        let key = TagKey::new(valid);
        if !seen.contains(&key) {
            seen.push(key);
            out.push(valid);
        }
    }
    Ok(out)
}

fn list_yaml(names: &[&str]) -> Option<String> {
    (!names.is_empty()).then(|| {
        let names: Vec<String> = names.iter().map(|name| scalar(name)).collect();
        format!("[{}]", names.join(", "))
    })
}

/// Sin comillas, `#d97706` sería un comentario de YAML (05 §7).
fn color_yaml(color: &TagColor) -> Result<String, PlanError> {
    let text = match color {
        TagColor::Named(text) | TagColor::Hex(text) => text,
    };
    if TagColor::parse(text).as_ref() != Some(color) {
        return Err(PlanError::InvalidValue(text.clone()));
    }
    Ok(match color {
        TagColor::Named(name) => scalar(name),
        TagColor::Hex(hex) => format!("\"{hex}\""),
    })
}

fn icon_yaml(icon: &TagIcon) -> Result<String, PlanError> {
    let text = match icon {
        TagIcon::Emoji(emoji) => emoji.clone(),
        TagIcon::Set { set, name } => format!("{set}:{name}"),
    };
    if TagIcon::parse(&text).as_ref() != Some(icon) {
        return Err(PlanError::InvalidValue(text));
    }
    Ok(scalar(&text))
}

/// `text` como escalar de YAML que se lee como el mismo texto: entre
/// comillas si sin ellas sería un número, un booleano, un nulo o un
/// comentario. Nada de lo que se escribe aquí lleva `"` ni `\`.
fn scalar(text: &str) -> String {
    let starts_like_other = text
        .chars()
        .next()
        .is_none_or(|c| c.is_ascii_digit() || "-+.#&*!|>%@`'\"?:,[]{}~".contains(c));
    let keyword = ["null", "true", "false"]
        .iter()
        .any(|word| text.eq_ignore_ascii_case(word));
    if starts_like_other || keyword {
        format!("\"{text}\"")
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// Lo que el scanner lee de vuelta de `key: <escalar>`.
    fn read_back(text: &str) -> Option<String> {
        let note = format!("---\naliases: [{}]\n---\n", scalar(text));
        let frontmatter = nota_syntax::scan(&note).frontmatter?;
        frontmatter.aliases.first().map(|value| value.text.clone())
    }

    #[rstest]
    #[case("rust", "rust")]
    #[case("2024", "\"2024\"")]
    #[case("007", "\"007\"")]
    #[case("-a", "\"-a\"")]
    #[case("null", "\"null\"")]
    #[case("True", "\"True\"")]
    #[case("🦀", "🦀")]
    #[case("#️⃣", "\"#️⃣\"")]
    #[case("tabler:book", "tabler:book")]
    #[case("canción", "canción")]
    fn scalars_read_back_as_written(#[case] text: &str, #[case] yaml: &str) {
        assert_eq!(scalar(text), yaml);
        assert_eq!(read_back(text).as_deref(), Some(text));
    }
}
