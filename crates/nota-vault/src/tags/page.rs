//! Las páginas de tag: `tags/<nombre>.md` con `color`, `icon`, `aliases`,
//! `parents` y `template` en el frontmatter (05 §7).

use std::ops::Range;

use nota_syntax::{Frontmatter, Value};
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory as _};

use crate::path::VaultPath;
use crate::tags::key::TagKey;
use crate::tags::layout::PageLayout;
use crate::tags::note::{Lines, TagOccurrence};

/// Un tag con página.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPage {
    pub path: VaultPath,
    pub key: TagKey,
    /// El nombre del archivo sin `.md`, como está escrito.
    pub name: String,
    pub color: Option<TagColor>,
    pub icon: Option<TagIcon>,
    /// Tal como los declara la página; el `TagGraph` decide cuáles aplican.
    pub aliases: Vec<TagOccurrence>,
    pub parents: Vec<TagOccurrence>,
    /// La plantilla de las notas nuevas del tag (06 §11): su ruta dentro de
    /// la carpeta de plantillas, sin `.md`.
    pub template: Option<String>,
    /// Valores que no se entienden; el resto de la página vale igual.
    pub invalid: Vec<InvalidValue>,
    /// Dónde escribir un campo sin tocar el resto.
    pub layout: PageLayout,
}

/// Un nombre de la paleta del tema (`naranja`) o un hex (`#d97706`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagColor {
    Named(String),
    Hex(String),
}

/// Un emoji o un ícono de un set (`tabler:book`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagIcon {
    Emoji(String),
    Set { set: String, name: String },
}

/// Un valor de la página que no se pudo usar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidValue {
    pub field: PageField,
    pub text: String,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageField {
    Color,
    Icon,
    Aliases,
    Parents,
    Template,
}

impl TagPage {
    /// La página de `name` a partir de su frontmatter, si tiene.
    pub(crate) fn new(
        path: VaultPath,
        name: &str,
        text: &str,
        frontmatter: Option<&Frontmatter>,
    ) -> Self {
        let mut page = Self {
            path,
            key: TagKey::new(name),
            name: name.to_owned(),
            color: None,
            icon: None,
            aliases: Vec::new(),
            parents: Vec::new(),
            template: None,
            invalid: Vec::new(),
            layout: PageLayout::new(text, frontmatter),
        };
        if let Some(frontmatter) = frontmatter {
            page.read(text, frontmatter);
        }
        page
    }

    fn read(&mut self, text: &str, frontmatter: &Frontmatter) {
        self.color = self.single(
            PageField::Color,
            frontmatter.color.as_ref(),
            TagColor::parse,
        );
        self.icon = self.single(PageField::Icon, frontmatter.icon.as_ref(), TagIcon::parse);
        let mut lines = Lines::new(text);
        self.aliases = self.tags(text, &mut lines, PageField::Aliases, &frontmatter.aliases);
        self.parents = self.tags(text, &mut lines, PageField::Parents, &frontmatter.parents);
        self.template = self.single(
            PageField::Template,
            frontmatter.template.as_ref(),
            parse_template,
        );
    }

    fn single<T>(
        &mut self,
        field: PageField,
        value: Option<&Value>,
        parse: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        let value = value?;
        let parsed = parse(&value.text);
        if parsed.is_none() {
            self.reject(field, value);
        }
        parsed
    }

    fn tags(
        &mut self,
        text: &str,
        lines: &mut Lines<'_>,
        field: PageField,
        values: &[Value],
    ) -> Vec<TagOccurrence> {
        let mut tags = Vec::new();
        for value in values {
            match value.as_tag() {
                Some(tag) => tags.push(TagOccurrence::from_value(text, &tag, lines)),
                None => self.reject(field, value),
            }
        }
        tags
    }

    fn reject(&mut self, field: PageField, value: &Value) {
        self.invalid.push(InvalidValue {
            field,
            text: value.text.clone(),
            span: value.span.clone(),
        });
    }
}

impl TagColor {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if let Some(digits) = text.strip_prefix('#') {
            let valid =
                matches!(digits.len(), 3 | 6) && digits.chars().all(|c| c.is_ascii_hexdigit());
            return valid.then(|| Self::Hex(text.to_owned()));
        }
        let mut chars = text.chars();
        let valid = chars.next().is_some_and(char::is_alphabetic)
            && chars.all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'));
        valid.then(|| Self::Named(text.to_owned()))
    }
}

impl TagIcon {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if let Some((set, name)) = text.split_once(':') {
            let part = |part: &str| {
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            };
            return (part(set) && part(name)).then(|| Self::Set {
                set: set.to_owned(),
                name: name.to_owned(),
            });
        }
        is_emoji(text).then(|| Self::Emoji(text.to_owned()))
    }
}

/// El nombre de una plantilla si es una ruta del vault sin `.md`
/// (`trabajo/semanal`): sin `..`, sin ser absoluta ni oculta. Sin espacios
/// alrededor, que el frontmatter recorta al leer.
#[must_use]
pub fn parse_template(text: &str) -> Option<String> {
    let valid = text.trim() == text && VaultPath::new(&format!("{text}.md")).is_ok();
    valid.then(|| text.to_owned())
}

/// Símbolos con sus modificadores: variaciones, ZWJ, etiquetas de banderas y
/// los dígitos de un keycap. Alcanza para decir que no es texto cualquiera.
fn is_emoji(text: &str) -> bool {
    let symbol =
        |c: char| !c.is_ascii() && c.general_category_group() == GeneralCategoryGroup::Symbol;
    text.chars().any(|c| symbol(c) || c == '\u{20E3}')
        && text.chars().all(|c| {
            symbol(c)
                || matches!(c, '0'..='9' | '#' | '*' | '\u{200D}' | '\u{20E3}' | '\u{FE0F}' | '\u{E0020}'..='\u{E007F}')
                || c.general_category_group() == GeneralCategoryGroup::Mark
        })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("naranja", Some(TagColor::Named("naranja".into())))]
    #[case("#d97706", Some(TagColor::Hex("#d97706".into())))]
    #[case("#fff", Some(TagColor::Hex("#fff".into())))]
    #[case("#ffff", None)]
    #[case("#zzz", None)]
    #[case("rojo oscuro", None)]
    #[case("1rojo", None)]
    fn colors(#[case] text: &str, #[case] color: Option<TagColor>) {
        assert_eq!(TagColor::parse(text), color);
    }

    #[rstest]
    #[case("🦀", true)]
    #[case("👨‍👩‍👧", true)]
    #[case("🇲🇽", true)]
    #[case("#️⃣", true)]
    #[case("❤️", true)]
    #[case("tabler:brand-rust", true)]
    #[case("ph:books", true)]
    #[case("tabler:", false)]
    #[case(":book", false)]
    #[case("libro", false)]
    #[case("a🦀", false)]
    #[case("12", false)]
    fn icons(#[case] text: &str, #[case] valid: bool) {
        assert_eq!(TagIcon::parse(text).is_some(), valid, "{text}");
    }

    #[rstest]
    #[case("reunión", true)]
    #[case("trabajo/semanal", true)]
    #[case("Con espacios: y # signos", true)]
    #[case("../fuera", false)]
    #[case("/absoluta", false)]
    #[case(".oculta", false)]
    #[case("a//b", false)]
    #[case(" trabajo", false)]
    #[case("trabajo ", false)]
    fn templates(#[case] text: &str, #[case] valid: bool) {
        assert_eq!(parse_template(text).is_some(), valid, "{text}");
    }

    #[test]
    fn a_page_reads_its_fields() {
        let text = "---\ncolor: naranja\nicon: tabler:brand-rust\naliases: [rs, \"#Rust-lang\"]\nparents: lenguajes\ntemplate: trabajo/semanal\n---\nCuerpo";
        let scan = nota_syntax::scan(text);
        let page = TagPage::new(
            VaultPath::new("tags/Rust.md").unwrap(),
            "Rust",
            text,
            scan.frontmatter.as_ref(),
        );
        assert_eq!(page.key.as_str(), "rust");
        assert_eq!(page.color, Some(TagColor::Named("naranja".into())));
        assert_eq!(
            page.icon,
            Some(TagIcon::Set {
                set: "tabler".into(),
                name: "brand-rust".into()
            })
        );
        let names = |tags: &[TagOccurrence]| -> Vec<String> {
            tags.iter()
                .map(|tag| text[tag.range.clone()].to_owned())
                .collect()
        };
        assert_eq!(names(&page.aliases), ["rs", "Rust-lang"]);
        assert_eq!(names(&page.parents), ["lenguajes"]);
        assert_eq!(page.template.as_deref(), Some("trabajo/semanal"));
        assert_eq!(page.invalid, []);
    }

    #[test]
    fn values_that_are_not_understood_are_reported() {
        let text = "---\ncolor: rojo oscuro\nicon: libro\naliases: [a/b]\nparents: [\"x y\"]\ntemplate: ../x\n---\n";
        let scan = nota_syntax::scan(text);
        let page = TagPage::new(
            VaultPath::new("tags/a.md").unwrap(),
            "a",
            text,
            scan.frontmatter.as_ref(),
        );
        let fields: Vec<PageField> = page.invalid.iter().map(|value| value.field).collect();
        assert_eq!(
            fields,
            [
                PageField::Color,
                PageField::Icon,
                PageField::Aliases,
                PageField::Parents,
                PageField::Template
            ]
        );
        assert_eq!((page.color, page.icon, page.template), (None, None, None));
    }
}
