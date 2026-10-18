//! Los candidatos del autocompletado de `#` (05 §5): cada tag con su forma
//! establecida, sus alias, su conteo, su color y su ícono. El filtrado
//! difuso es de `metalmind-search`; aquí está lo que hay que ofrecer.

use metalmind_syntax::TagPlace;

use crate::tags::Tags;
use crate::tags::key::TagKey;
use crate::tags::metadata::yaml_scalar;
use crate::tags::page::{TagColor, TagIcon};

/// Un tag que se puede ofrecer al escribir `#`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCompletion {
    pub key: TagKey,
    /// La forma establecida (05 §6): la que se inserta, para que no nazcan
    /// variantes nuevas.
    pub name: String,
    /// Los alias que aplican, como los escribe la página: escribir `#rs`
    /// también encuentra "Rust (alias rs)".
    pub aliases: Vec<String>,
    /// Notas distintas con el tag, sus alias o sus descendientes.
    pub count: usize,
    pub color: Option<TagColor>,
    pub icon: Option<TagIcon>,
}

impl TagCompletion {
    /// El texto que va en el rango `replace` de un `metalmind_syntax::TagSpot`:
    /// el nombre en el cuerpo y, en el YAML, un escalar que se lea igual.
    #[must_use]
    pub fn text(&self, place: TagPlace) -> String {
        match place {
            TagPlace::Inline => self.name.clone(),
            TagPlace::Tags | TagPlace::Aliases | TagPlace::Parents => yaml_scalar(&self.name),
        }
    }
}

impl Tags {
    /// Todos los tags del vault, de más notas a menos y, a igual conteo, por
    /// nombre. Los alias no son candidatos propios: van en su tag.
    #[must_use]
    pub fn completions(&self) -> Vec<TagCompletion> {
        let tally = self.tally(|_| true);
        let mut completions: Vec<TagCompletion> = tally
            .keys
            .iter()
            .map(|&key| {
                let page = self.graph.page(key);
                let aliases = page
                    .into_iter()
                    .flat_map(|page| &page.aliases)
                    .filter(|alias| self.graph.alias_of(&alias.key) == Some(key))
                    .map(|alias| alias.spelling.clone())
                    .collect();
                TagCompletion {
                    key: key.clone(),
                    name: self.display_name(key).unwrap_or(key.as_str()).to_owned(),
                    aliases,
                    count: tally.count(key),
                    color: page.and_then(|page| page.color.clone()),
                    icon: page.and_then(|page| page.icon.clone()),
                }
            })
            .collect();
        completions.sort_by_cached_key(|completion| {
            (
                std::cmp::Reverse(completion.count),
                completion.name.to_lowercase(),
                completion.name.clone(),
            )
        });
        completions
    }
}
