//! Los campos de la página de un tag que se editan desde la UI (05 §3): lo
//! que tienen ahora, como se escriben en una pregunta o tras `:`, y el
//! cambio que pide lo escrito.

use metalmind_vault::tags::{PageChange, TagIcon, TagKey, TagPage, Tags, parse_template};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagField {
    Aliases,
    Parents,
    Icon,
    Template,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    #[error("`{0}` no es un ícono: va un emoji o `set:nombre`")]
    Icon(String),
    #[error("`{0}` no es el nombre de una plantilla")]
    Template(String),
}

impl TagField {
    /// Cómo lo nombran la pregunta y la vista previa.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Aliases => "Alias",
            Self::Parents => "Padres",
            Self::Icon => "Ícono",
            Self::Template => "Plantilla",
        }
    }

    /// Qué se escribe, para la pregunta del panel.
    #[must_use]
    pub fn hint(self) -> &'static str {
        match self {
            Self::Aliases | Self::Parents => "separados por coma",
            Self::Icon => "emoji o set:nombre",
            Self::Template => "nombre de la plantilla",
        }
    }

    #[must_use]
    pub fn usage(self) -> &'static str {
        match self {
            Self::Aliases => ":tagaliases #tag [alias…]",
            Self::Parents => ":tagparents #tag [padre…]",
            Self::Icon => ":tagicon #tag [ícono]",
            Self::Template => ":tagtemplate #tag [plantilla]",
        }
    }

    /// Lo que tiene la página de `tag`, escrito como se pide en la
    /// pregunta; vacío si no tiene página o el campo.
    #[must_use]
    pub fn current(self, tags: &Tags, tag: &str) -> String {
        let graph = tags.graph();
        let page = TagKey::parse(tag).and_then(|key| graph.page(graph.resolve(&key)));
        page.map(|page| self.written(page)).unwrap_or_default()
    }

    fn written(self, page: &TagPage) -> String {
        let names = |list: &[metalmind_vault::tags::TagOccurrence]| {
            let names: Vec<&str> = list.iter().map(|tag| tag.spelling.as_str()).collect();
            names.join(", ")
        };
        match self {
            Self::Aliases => names(&page.aliases),
            Self::Parents => names(&page.parents),
            Self::Icon => match &page.icon {
                Some(TagIcon::Emoji(emoji)) => emoji.clone(),
                Some(TagIcon::Set { set, name }) => format!("{set}:{name}"),
                None => String::new(),
            },
            Self::Template => page.template.clone().unwrap_or_default(),
        }
    }

    /// El cambio que pide `text`. Vacío quita el campo; los nombres de
    /// tag los valida el plan, que dice cuál no vale.
    ///
    /// # Errors
    /// Si no es un ícono o el nombre de una plantilla.
    pub fn change(self, text: &str) -> Result<PageChange, FieldError> {
        let text = text.trim();
        let names = || {
            text.split([',', ' '])
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let given = (!text.is_empty()).then_some(text);
        Ok(match self {
            Self::Aliases => PageChange::Aliases(names()),
            Self::Parents => PageChange::Parents(names()),
            Self::Icon => PageChange::Icon(
                given
                    .map(|text| {
                        TagIcon::parse(text).ok_or_else(|| FieldError::Icon(text.to_owned()))
                    })
                    .transpose()?,
            ),
            Self::Template => PageChange::Template(
                given
                    .map(|text| {
                        parse_template(text).ok_or_else(|| FieldError::Template(text.to_owned()))
                    })
                    .transpose()?,
            ),
        })
    }
}
