//! Los campos del frontmatter que nota entiende, leídos con serde-saphyr
//! (YAML 1.2). Se lee a mano con `deserialize_any` para que un campo de un
//! tipo raro solo invalide ese campo y no todo el frontmatter.

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_saphyr::localizer::Localizer;
use serde_saphyr::{
    Error, Location, MessageFormatter, SnippetMode, Span, Spanned, UserMessageFormatter,
};

/// Los escalares de YAML 1.2 como texto, envueltos con `$wrap`.
macro_rules! visit_scalars {
    ($value:ty, $wrap:expr) => {
        fn visit_bool<E: de::Error>(self, value: bool) -> Result<$value, E> {
            Ok(($wrap)(value.to_string()))
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<$value, E> {
            Ok(($wrap)(value.to_string()))
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<$value, E> {
            Ok(($wrap)(value.to_string()))
        }

        fn visit_f64<E: de::Error>(self, value: f64) -> Result<$value, E> {
            Ok(($wrap)(value.to_string()))
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<$value, E> {
            Ok(($wrap)(value.to_owned()))
        }
    };
}

/// Un escalar como texto, con su rango en bytes dentro del YAML.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Item {
    pub text: String,
    pub span: Range<usize>,
}

/// Un campo tal como venía: `null`, lista, un solo texto, o algo que no es
/// ninguno de esos. `null` no es una lista vacía: `template: []` es un error.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Field {
    Null,
    List(Vec<Item>),
    Text(Item),
    Invalid,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Fields {
    pub tags: Option<Field>,
    pub aliases: Option<Field>,
    pub parents: Option<Field>,
    pub cssclasses: Option<Field>,
    pub color: Option<Field>,
    pub icon: Option<Field>,
    pub template: Option<Field>,
    /// Las claves del primer nivel, en orden; `None` si alguna no es un
    /// escalar (`? [a]`).
    pub keys: Option<Vec<Item>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReadError {
    Syntax { at: usize, message: String },
    NotMapping,
}

/// Lee los campos de `yaml`; los rangos y `at` en un error de sintaxis son
/// relativos a él.
pub(super) fn read(yaml: &str) -> Result<Fields, ReadError> {
    let document: Document = serde_saphyr::from_str_with_options(yaml, options())
        .map_err(|error| syntax_error(yaml, &error))?;
    let fields = document.0.ok_or(ReadError::NotMapping)?;
    let field = |field: Option<RawField>| field.map(|field| field.into_field(yaml));
    Ok(Fields {
        tags: field(fields.tags),
        aliases: field(fields.aliases),
        parents: field(fields.parents),
        cssclasses: field(fields.cssclasses),
        color: field(fields.color),
        icon: field(fields.icon),
        template: field(fields.template),
        keys: fields
            .keys
            .into_iter()
            .map(|key| {
                Some(Item {
                    text: key.value.0?,
                    span: byte_range(yaml, key.referenced.span()),
                })
            })
            .collect(),
    })
}

/// Un rango de serde-saphyr en bytes. Con un `&str` siempre trae los bytes;
/// si no, se cuentan desde los caracteres.
fn byte_range(yaml: &str, span: Span) -> Range<usize> {
    let bytes = span
        .byte_offset()
        .zip(span.byte_len())
        .and_then(|(start, len)| Some((usize::try_from(start).ok()?, usize::try_from(len).ok()?)));
    if let Some((start, len)) = bytes {
        return start..start + len;
    }
    let byte = |chars: u64| {
        usize::try_from(chars)
            .ok()
            .and_then(|chars| yaml.char_indices().nth(chars))
            .map_or(yaml.len(), |(byte, _)| byte)
    };
    byte(span.offset())..byte(span.offset() + span.len())
}

fn options() -> serde_saphyr::Options {
    // Como el config: solo `true` y `false` son booleanos, y un `.nan` es su
    // texto en vez de un error que se lleve el frontmatter entero.
    serde_saphyr::options! {
        strict_booleans: true,
        non_finite_float_policy: serde_saphyr::NonFiniteFloatPolicy::AsString,
    }
}

fn syntax_error(yaml: &str, error: &Error) -> ReadError {
    // El span de serde-saphyr cuenta caracteres, no bytes.
    let at = error
        .location()
        .and_then(|location| usize::try_from(location.span().offset()).ok())
        .and_then(|chars| yaml.char_indices().nth(chars))
        .map_or(yaml.len(), |(byte, _)| byte);
    ReadError::Syntax {
        at,
        message: error.render_with_options(serde_saphyr::render_options! {
            formatter: &WithoutLocation,
            snippets: SnippetMode::Off,
        }),
    }
}

/// El mensaje para el usuario sin línea ni columna: el problema ya lleva su
/// posición en bytes.
struct WithoutLocation;

impl Localizer for WithoutLocation {
    fn attach_location<'a>(&self, base: Cow<'a, str>, _: Location) -> Cow<'a, str> {
        base
    }
}

impl MessageFormatter for WithoutLocation {
    fn localizer(&self) -> &dyn Localizer {
        self
    }

    fn format_message<'a>(&self, error: &'a Error) -> Cow<'a, str> {
        UserMessageFormatter.format_message(error)
    }
}

/// Los campos como los entrega serde-saphyr, con sus rangos sin convertir.
#[derive(Default)]
struct RawFields {
    tags: Option<RawField>,
    aliases: Option<RawField>,
    parents: Option<RawField>,
    cssclasses: Option<RawField>,
    color: Option<RawField>,
    icon: Option<RawField>,
    template: Option<RawField>,
    keys: Vec<Spanned<Scalar>>,
}

/// El valor de un campo y dónde está.
struct RawField {
    shape: Shape,
    span: Span,
}

enum Shape {
    Null,
    List(Vec<Spanned<Scalar>>),
    Text(String),
    Invalid,
}

impl RawField {
    fn into_field(self, yaml: &str) -> Field {
        match self.shape {
            Shape::List(items) => Field::List(
                items
                    .into_iter()
                    .filter_map(|item| {
                        Some(Item {
                            text: item.value.0?,
                            span: byte_range(yaml, item.referenced.span()),
                        })
                    })
                    .collect(),
            ),
            Shape::Text(text) => Field::Text(Item {
                text,
                span: byte_range(yaml, self.span),
            }),
            Shape::Null => Field::Null,
            Shape::Invalid => Field::Invalid,
        }
    }
}

/// El documento entero: `None` si no es un mapa de claves. Vacío o `null`
/// es un frontmatter sin campos.
struct Document(Option<RawFields>);

impl<'de> Deserialize<'de> for Document {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DocumentVisitor)
    }
}

struct DocumentVisitor;

impl<'de> Visitor<'de> for DocumentVisitor {
    type Value = Document;

    visit_scalars!(Document, |_| Document(None));

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("un mapa de claves")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Document, E> {
        Ok(Document(Some(RawFields::default())))
    }

    fn visit_none<E: de::Error>(self) -> Result<Document, E> {
        self.visit_unit()
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Document, A::Error> {
        let mut fields = RawFields::default();
        while let Some(key) = map.next_key::<Spanned<Scalar>>()? {
            let name = key.value.0.clone();
            fields.keys.push(key);
            let slot = match name.as_deref() {
                Some("tags") => &mut fields.tags,
                Some("aliases") => &mut fields.aliases,
                Some("parents") => &mut fields.parents,
                Some("cssclasses") => &mut fields.cssclasses,
                Some("color") => &mut fields.color,
                Some("icon") => &mut fields.icon,
                Some("template") => &mut fields.template,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                    continue;
                }
            };
            let value: Spanned<Shape> = map.next_value()?;
            *slot = Some(RawField {
                shape: value.value,
                span: value.referenced.span(),
            });
        }
        Ok(Document(Some(fields)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Document, A::Error> {
        IgnoredAny.visit_seq(seq)?;
        Ok(Document(None))
    }
}

impl<'de> Deserialize<'de> for Shape {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ShapeVisitor)
    }
}

struct ShapeVisitor;

impl<'de> Visitor<'de> for ShapeVisitor {
    type Value = Shape;

    visit_scalars!(Shape, Shape::Text);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("una lista o un texto")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Shape, E> {
        Ok(Shape::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Shape, E> {
        self.visit_unit()
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Shape, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Shape::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Shape, A::Error> {
        IgnoredAny.visit_map(map)?;
        Ok(Shape::Invalid)
    }
}

/// Un escalar como texto. `None` para `null` y para listas o mapas, que se
/// saltan: una clave compleja o un elemento anidado no aportan nada.
struct Scalar(Option<String>);

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ScalarVisitor)
    }
}

struct ScalarVisitor;

impl<'de> Visitor<'de> for ScalarVisitor {
    type Value = Scalar;

    visit_scalars!(Scalar, |text| Scalar(Some(text)));

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("un escalar")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Scalar, E> {
        Ok(Scalar(None))
    }

    fn visit_none<E: de::Error>(self) -> Result<Scalar, E> {
        self.visit_unit()
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Scalar, A::Error> {
        IgnoredAny.visit_seq(seq)?;
        Ok(Scalar(None))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Scalar, A::Error> {
        IgnoredAny.visit_map(map)?;
        Ok(Scalar(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    /// Cada valor con el texto del YAML que ocupa.
    fn tags(yaml: &str) -> Option<Vec<(String, &str)>> {
        let item = |item: Item| (item.text, &yaml[item.span]);
        match read(yaml).ok()?.tags? {
            Field::List(items) => Some(items.into_iter().map(item).collect()),
            Field::Text(text) => Some(vec![item(text)]),
            Field::Null => Some(Vec::new()),
            Field::Invalid => None,
        }
    }

    #[rstest]
    #[case::list("tags: [a, b]", &[("a", "a"), ("b", "b")])]
    #[case::text("tags: a b", &[("a b", "a b")])]
    #[case::null("tags:", &[])]
    #[case::number("tags: 2024", &[("2024", "2024")])]
    #[case::norway("tags: [no, yes, on]", &[("no", "no"), ("yes", "yes"), ("on", "on")])]
    #[case::nan("tags: .nan", &[(".nan", ".nan")])]
    #[case::nested_items("tags: [a, [b], {c: d}, ~]", &[("a", "a")])]
    #[case::quoted("tags: [\"#a\", 'b''c']", &[("#a", "\"#a\""), ("b'c", "'b''c'")])]
    #[case::block("tags:\n  - a\n  - ñú\n", &[("a", "a"), ("ñú", "ñú")])]
    #[case::folded("tags:\n  - a\n    b\n", &[("a b", "a\n    b")])]
    fn field_values(#[case] yaml: &str, #[case] values: &[(&str, &str)]) {
        let values: Vec<(String, &str)> = values
            .iter()
            .map(|&(text, source)| (text.to_owned(), source))
            .collect();
        assert_eq!(tags(yaml), Some(values));
    }

    #[test]
    fn a_mapping_is_not_a_field() {
        assert_eq!(read("tags: {a: b}").unwrap().tags, Some(Field::Invalid));
        assert_eq!(read("tags:").unwrap().tags, Some(Field::Null));
        assert_eq!(
            read("tags: []").unwrap().tags,
            Some(Field::List(Vec::new()))
        );
    }

    #[test]
    fn other_keys_of_any_shape_are_ignored() {
        let yaml = "? [complex]\n: key\n1: x\ntitle: {a: [b]}\naliases: [x]";
        let fields = read(yaml).unwrap();
        let x = yaml.rfind('x').unwrap();
        assert_eq!(
            fields.aliases,
            Some(Field::List(vec![Item {
                text: "x".into(),
                span: x..x + 1
            }]))
        );
    }

    #[rstest]
    #[case::empty("")]
    #[case::only_comment("# nada")]
    #[case::null("~")]
    fn an_empty_document_has_no_fields(#[case] yaml: &str) {
        let empty = Fields {
            keys: Some(Vec::new()),
            ..Fields::default()
        };
        assert_eq!(read(yaml), Ok(empty));
    }

    #[rstest]
    #[case::list("- a\n- b")]
    #[case::text("solo texto")]
    #[case::number("42")]
    fn a_document_that_is_not_a_mapping(#[case] yaml: &str) {
        assert_eq!(read(yaml), Err(ReadError::NotMapping));
    }

    #[test]
    fn a_syntax_error_points_inside_the_yaml() {
        let yaml = "título: ñandú\ntags: [sin, cerrar\naliases: \"mal\n";
        let Err(ReadError::Syntax { at, message }) = read(yaml) else {
            panic!("se esperaba un error de sintaxis");
        };
        assert_eq!(&yaml[at..], ": \"mal\n");
        assert!(!message.contains("line"), "{message}");
    }
}
