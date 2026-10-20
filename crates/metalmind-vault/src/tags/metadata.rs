//! Cambiar la metadata de un tag desde una acción (05 §3): color, ícono,
//! alias, padres y plantilla. Se escribe en el frontmatter de su página tocando solo
//! ese campo, y si el tag no tiene página, Metalmind la crea.

use metalmind_syntax::note_name;
use unicode_normalization::UnicodeNormalization as _;

use crate::changes::{Changes, NewFile};
use crate::edit::{FileEdits, TextEdit};
use crate::path::VaultPath;
use crate::tags::Tags;
use crate::tags::key::{TagKey, tag_name};
use crate::tags::layout::{PageEntry, PageLayout};
use crate::tags::page::{PageField, TagColor, TagIcon, TagPage, parse_template};
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
    /// El nombre de una plantilla (06 §11).
    Template(Option<String>),
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
    /// `tags::set_icon`, `tags::edit_aliases`, `tags::edit_parents`,
    /// `tags::set_template`). Un
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
            Some(page) => edit_page(page, field, value.as_deref())?,
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
        self.page_file(name)
            .ok_or_else(|| PlanError::InvalidName(name.to_owned()))
    }

    /// `tags/<name>.md`, con el nombre en NFC (05 §1).
    pub(crate) fn page_file(&self, name: &str) -> Option<VaultPath> {
        let name: String = name.nfc().collect();
        self.folder.join(&note_name::with_extension(&name)).ok()
    }

    fn create_page(&self, key: &TagKey, tag: &str, text: String) -> Result<TagPlan, PlanError> {
        let path = self.new_page_path(key, tag)?;
        let notices = not_portable(key, &path).into_iter().collect();
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
            PageChange::Template(template) => (
                PageField::Template,
                template.as_deref().map(template_yaml).transpose()?,
            ),
        })
    }

    /// Lo que la vista previa avisa de los alias nuevos de `key`.
    fn alias_notices(&self, key: &TagKey, aliases: &[String]) -> Vec<PlanNotice> {
        aliases
            .iter()
            .filter_map(|alias| TagKey::parse(alias))
            .filter_map(|alias| self.alias_notice(&alias, key))
            .collect()
    }
}

/// El aviso para una página cuyo nombre Windows reserva (`con`, `nul`…).
pub(crate) fn not_portable(key: &TagKey, path: &VaultPath) -> Option<PlanNotice> {
    RESERVED_ON_WINDOWS
        .contains(&key.as_str())
        .then(|| PlanNotice::NotPortable { path: path.clone() })
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
            Self::Template => "template",
        }
    }
}

/// La edición de un campo en una página que ya existe.
fn edit_page(page: &TagPage, field: PageField, value: Option<&str>) -> Result<TagPlan, PlanError> {
    let key = field.key();
    let edits = match (&page.layout, value) {
        (PageLayout::Unreadable, _) => return Err(PlanError::UneditablePage(page.path.clone())),
        (PageLayout::Bare { .. }, None) => Vec::new(),
        (&PageLayout::Bare { at, crlf }, Some(value)) => {
            let eol = eol(crlf);
            vec![TextEdit {
                range: at..at,
                old: String::new(),
                new: format!("---{eol}{key}: {value}{eol}---{eol}"),
                line: 0,
            }]
        }
        (
            PageLayout::Block {
                entries,
                end,
                end_line,
                crlf,
            },
            value,
        ) => {
            let line = value.map(|value| format!("{key}: {value}{}", eol(*crlf)));
            block_edit(entries, key, (*end, *end_line), line)
                .into_iter()
                .collect()
        }
    };
    let mut plan = TagPlan::default();
    if !edits.is_empty() {
        plan.changes.edits.push(FileEdits {
            path: page.path.clone(),
            edits,
        });
    }
    Ok(plan)
}

/// En un mapa en bloque, `line` reemplaza el campo o va antes del `---` de
/// cierre (`end`, con su línea); `None` lo quita. Una clave repetida es YAML
/// inválido, así que hay a lo sumo una.
fn block_edit(
    entries: &[PageEntry],
    key: &str,
    (end, end_line): (usize, usize),
    line: Option<String>,
) -> Option<TextEdit> {
    match (entries.iter().find(|entry| entry.key == key), line) {
        (Some(entry), line) => Some(TextEdit {
            range: entry.span.clone(),
            old: entry.text.clone(),
            new: line.unwrap_or_default(),
            line: entry.line,
        }),
        (None, Some(new)) => Some(TextEdit {
            range: end..end,
            old: String::new(),
            new,
            line: end_line,
        }),
        (None, None) => None,
    }
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
        let names: Vec<String> = names.iter().map(|name| yaml_scalar(name)).collect();
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
        TagColor::Named(name) => yaml_scalar(name),
        TagColor::Hex(hex) => format!("\"{hex}\""),
    })
}

fn template_yaml(name: &str) -> Result<String, PlanError> {
    parse_template(name)
        .map(|name| yaml_scalar(&name))
        .ok_or_else(|| PlanError::InvalidValue(name.to_owned()))
}

fn icon_yaml(icon: &TagIcon) -> Result<String, PlanError> {
    let text = match icon {
        TagIcon::Emoji(emoji) => emoji.clone(),
        TagIcon::Set { set, name } => format!("{set}:{name}"),
    };
    if TagIcon::parse(&text).as_ref() != Some(icon) {
        return Err(PlanError::InvalidValue(text));
    }
    Ok(yaml_scalar(&text))
}

/// `text` como escalar de YAML que se lee como el mismo texto, también
/// dentro de una lista de flujo: entre comillas si sin ellas sería un
/// número, un booleano, un nulo, un comentario o cortaría la lista.
#[must_use]
pub fn yaml_scalar(text: &str) -> String {
    let starts_like_other = text
        .chars()
        .next()
        .is_none_or(|c| c.is_ascii_digit() || "-+.#&*!|>%@`'\"?:,[]{}~".contains(c));
    let keyword = ["null", "true", "false"]
        .iter()
        .any(|word| text.eq_ignore_ascii_case(word));
    let breaks_plain = text.contains([',', '[', ']', '{', '}', '"', '\\'])
        || text.contains(": ")
        || text.contains(" #")
        || text.ends_with([':', ' ']);
    if starts_like_other || keyword || breaks_plain {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
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
        let note = format!("---\naliases: [{}]\n---\n", yaml_scalar(text));
        let frontmatter = metalmind_syntax::scan(&note).frontmatter?;
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
    #[case("trabajo/semanal", "trabajo/semanal")]
    #[case("a: b", "\"a: b\"")]
    #[case("a #b", "\"a #b\"")]
    #[case("a,b", "\"a,b\"")]
    #[case("di\"ce", "\"di\\\"ce\"")]
    #[case("c:\\x", "\"c:\\\\x\"")]
    fn scalars_read_back_as_written(#[case] text: &str, #[case] yaml: &str) {
        assert_eq!(yaml_scalar(text), yaml);
        assert_eq!(read_back(text).as_deref(), Some(text));
    }
}
