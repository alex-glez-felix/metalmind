//! El frontmatter leído con serde-saphyr (YAML 1.2): cada valor entero y,
//! con su rango, los campos que Metalmind entiende. Se lee a mano con
//! `deserialize_any` para que un campo de un tipo raro solo invalide ese
//! campo y no todo el frontmatter.

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_saphyr::localizer::Localizer;
use serde_saphyr::{
    Error, Location, MessageFormatter, SnippetMode, Span, Spanned, UserMessageFormatter,
};

use super::property::{Property, PropertyValue};

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
    /// Las de clave escalar con su valor, en orden.
    pub properties: Vec<Property>,
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
        properties: fields.properties,
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
    properties: Vec<Property>,
}

/// El valor de un campo y dónde está.
struct RawField {
    node: Node,
    span: Span,
}

impl RawField {
    /// Un elemento anidado de una lista (`[a, [b]]`) no aporta nada y se salta.
    fn into_field(self, yaml: &str) -> Field {
        match self.node {
            Node::List(items) => Field::List(
                items
                    .into_iter()
                    .filter_map(|item| match item.value {
                        Node::Scalar(text) => Some(Item {
                            text,
                            span: byte_range(yaml, item.referenced.span()),
                        }),
                        Node::Null | Node::List(_) | Node::Map(_) => None,
                    })
                    .collect(),
            ),
            Node::Scalar(text) => Field::Text(Item {
                text,
                span: byte_range(yaml, self.span),
            }),
            Node::Null => Field::Null,
            Node::Map(_) => Field::Invalid,
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
            let value: Spanned<Node> = map.next_value()?;
            let Some(name) = name else {
                continue;
            };
            fields.properties.push(Property {
                key: name.clone(),
                value: value.value.to_property(),
            });
            let slot = match name.as_str() {
                "tags" => &mut fields.tags,
                "aliases" => &mut fields.aliases,
                "parents" => &mut fields.parents,
                "cssclasses" => &mut fields.cssclasses,
                "color" => &mut fields.color,
                "icon" => &mut fields.icon,
                "template" => &mut fields.template,
                _ => continue,
            };
            *slot = Some(RawField {
                node: value.value,
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

/// Un valor cualquiera, con el rango de cada elemento de una lista: de ahí
/// salen tanto la [`Property`] como el campo que Metalmind entiende.
enum Node {
    Null,
    Scalar(String),
    List(Vec<Spanned<Node>>),
    /// Las claves que no son un escalar quedan como `None`.
    Map(Vec<(Option<String>, Node)>),
}

impl Node {
    fn to_property(&self) -> PropertyValue {
        match self {
            Self::Null => PropertyValue::Null,
            Self::Scalar(text) => PropertyValue::Text(text.clone()),
            Self::List(items) => {
                PropertyValue::List(items.iter().map(|item| item.value.to_property()).collect())
            }
            Self::Map(entries) => PropertyValue::Map(
                entries
                    .iter()
                    .filter_map(|(key, value)| {
                        Some(Property {
                            key: key.clone()?,
                            value: value.to_property(),
                        })
                    })
                    .collect(),
            ),
        }
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    visit_scalars!(Node, Node::Scalar);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("un valor de YAML")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Node, E> {
        self.visit_unit()
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Node::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut entries = Vec::new();
        while let Some(Scalar(key)) = map.next_key()? {
            entries.push((key, map.next_value()?));
        }
        Ok(Node::Map(entries))
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

    fn text(text: &str) -> PropertyValue {
        PropertyValue::Text(text.to_owned())
    }

    fn property(key: &str, value: PropertyValue) -> Property {
        Property {
            key: key.to_owned(),
            value,
        }
    }

    /// Cada clave escalar con su valor entero, también las que Metalmind no
    /// entiende y las de tipos que invalidan un campo suyo.
    #[test]
    fn every_key_keeps_its_whole_value() {
        let yaml = "título: Ñandú\nfecha: 2024-10-01\nlisto: true\n? [complex]\n: x\n\
                    vacío:\ntags: [a, [b], ~]\nautor: {nombre: Ana, 3: tres, ? [x]: y}\n\
                    ref: &r 7\notra: *r\n";
        let properties = read(yaml).unwrap().properties;
        let list = PropertyValue::List(vec![
            text("a"),
            PropertyValue::List(vec![text("b")]),
            PropertyValue::Null,
        ]);
        let author = PropertyValue::Map(vec![
            property("nombre", text("Ana")),
            property("3", text("tres")),
        ]);
        assert_eq!(
            properties,
            [
                property("título", text("Ñandú")),
                property("fecha", text("2024-10-01")),
                property("listo", text("true")),
                property("vacío", PropertyValue::Null),
                property("tags", list),
                property("autor", author),
                property("ref", text("7")),
                property("otra", text("7")),
            ]
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
