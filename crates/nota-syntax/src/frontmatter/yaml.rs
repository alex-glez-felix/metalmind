//! Los campos del frontmatter que nota entiende, leídos con serde-saphyr
//! (YAML 1.2). Se lee a mano con `deserialize_any` para que un campo de un
//! tipo raro solo invalide ese campo y no todo el frontmatter.

use std::borrow::Cow;
use std::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_saphyr::localizer::Localizer;
use serde_saphyr::{Error, Location, MessageFormatter, SnippetMode, UserMessageFormatter};

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

/// Un campo tal como venía: lista, texto que hay que partir, o algo que no
/// es ninguno de los dos.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Field {
    List(Vec<String>),
    Text(String),
    Invalid,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Fields {
    pub tags: Option<Field>,
    pub aliases: Option<Field>,
    pub cssclasses: Option<Field>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReadError {
    Syntax { at: usize, message: String },
    NotMapping,
}

/// Lee los campos de `yaml`; `at` en un error de sintaxis es relativo a él.
pub(super) fn read(yaml: &str) -> Result<Fields, ReadError> {
    let document: Document = serde_saphyr::from_str_with_options(yaml, options())
        .map_err(|error| syntax_error(yaml, &error))?;
    document.0.ok_or(ReadError::NotMapping)
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

/// El documento entero: `None` si no es un mapa de claves. Vacío o `null`
/// es un frontmatter sin campos.
struct Document(Option<Fields>);

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
        Ok(Document(Some(Fields::default())))
    }

    fn visit_none<E: de::Error>(self) -> Result<Document, E> {
        self.visit_unit()
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Document, A::Error> {
        let mut fields = Fields::default();
        while let Some(Scalar(key)) = map.next_key()? {
            let slot = match key.as_deref() {
                Some("tags") => &mut fields.tags,
                Some("aliases") => &mut fields.aliases,
                Some("cssclasses") => &mut fields.cssclasses,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                    continue;
                }
            };
            *slot = Some(map.next_value()?);
        }
        Ok(Document(Some(fields)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Document, A::Error> {
        IgnoredAny.visit_seq(seq)?;
        Ok(Document(None))
    }
}

impl<'de> Deserialize<'de> for Field {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(FieldVisitor)
    }
}

struct FieldVisitor;

impl<'de> Visitor<'de> for FieldVisitor {
    type Value = Field;

    visit_scalars!(Field, Field::Text);

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("una lista o un texto")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Field, E> {
        Ok(Field::List(Vec::new()))
    }

    fn visit_none<E: de::Error>(self) -> Result<Field, E> {
        self.visit_unit()
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Field, A::Error> {
        let mut items = Vec::new();
        while let Some(Scalar(item)) = seq.next_element()? {
            items.extend(item);
        }
        Ok(Field::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Field, A::Error> {
        IgnoredAny.visit_map(map)?;
        Ok(Field::Invalid)
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

    fn tags(yaml: &str) -> Option<Field> {
        read(yaml).map(|fields| fields.tags).ok().flatten()
    }

    #[rstest]
    #[case::list("tags: [a, b]", Field::List(vec!["a".into(), "b".into()]))]
    #[case::text("tags: a b", Field::Text("a b".into()))]
    #[case::null("tags:", Field::List(Vec::new()))]
    #[case::number("tags: 2024", Field::Text("2024".into()))]
    #[case::norway("tags: [no, yes, on]", Field::List(vec!["no".into(), "yes".into(), "on".into()]))]
    #[case::nan("tags: .nan", Field::Text(".nan".into()))]
    #[case::nested_items("tags: [a, [b], {c: d}, ~]", Field::List(vec!["a".into()]))]
    #[case::mapping("tags: {a: b}", Field::Invalid)]
    fn field_shapes(#[case] yaml: &str, #[case] field: Field) {
        assert_eq!(tags(yaml), Some(field));
    }

    #[test]
    fn other_keys_of_any_shape_are_ignored() {
        let fields = read("? [complex]\n: key\n1: x\ntitle: {a: [b]}\naliases: [x]").unwrap();
        assert_eq!(fields.aliases, Some(Field::List(vec!["x".into()])));
    }

    #[rstest]
    #[case::empty("")]
    #[case::only_comment("# nada")]
    #[case::null("~")]
    fn an_empty_document_has_no_fields(#[case] yaml: &str) {
        assert_eq!(read(yaml), Ok(Fields::default()));
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
