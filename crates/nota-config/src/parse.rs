//! Lectura de TOML a tipos de serde. Una clave desconocida es un aviso y no
//! tumba el archivo: un typo no debe costar el resto de la config.

use serde::de::DeserializeOwned;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::diagnostic::{Diagnostic, Location};

/// El valor leído y los avisos por claves ignoradas.
///
/// # Errors
/// Si el TOML está roto o un valor no tiene el tipo esperado.
pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> Result<(T, Vec<Diagnostic>), Diagnostic> {
    let table = DeTable::parse(text).map_err(|error| to_diagnostic(text, &error))?;
    let mut ignored = Vec::new();
    let value = serde_ignored::deserialize(toml::Deserializer::from(table.clone()), |path| {
        ignored.push(segments(&path));
    })
    .map_err(|error| to_diagnostic(text, &error))?;
    let warnings = ignored
        .into_iter()
        .map(|keys| {
            Diagnostic::warning(format!("clave desconocida `{}`, se ignora", keys.join(".")))
                .at(key_span(&table, &keys).map(|start| Location::of(text, start)))
        })
        .collect();
    Ok((value, warnings))
}

fn to_diagnostic(text: &str, error: &toml::de::Error) -> Diagnostic {
    Diagnostic::error(error.message().trim_end())
        .at(error.span().map(|span| Location::of(text, span.start)))
}

fn segments(path: &serde_ignored::Path<'_>) -> Vec<String> {
    use serde_ignored::Path;
    let (parent, key) = match path {
        Path::Root => return Vec::new(),
        Path::Seq { parent, index } => (parent, index.to_string()),
        Path::Map { parent, key } => (parent, key.clone()),
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => {
            return segments(parent);
        }
    };
    let mut keys = segments(parent);
    keys.push(key);
    keys
}

/// Dónde empieza la última clave de `keys` en el texto.
fn key_span(table: &Spanned<DeTable<'_>>, keys: &[String]) -> Option<usize> {
    let (last, parents) = keys.split_last()?;
    let mut current = table.get_ref();
    for key in parents {
        match lookup(current, key)?.1.get_ref() {
            DeValue::Table(inner) => current = inner,
            _ => return None,
        }
    }
    lookup(current, last).map(|(key, _)| key.span().start)
}

fn lookup<'t, 'i>(
    table: &'t DeTable<'i>,
    key: &str,
) -> Option<(
    &'t Spanned<toml::de::DeString<'i>>,
    &'t Spanned<DeValue<'i>>,
)> {
    table.iter().find(|(name, _)| name.get_ref() == key)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::diagnostic::Severity;

    #[derive(Debug, Default, Deserialize, PartialEq)]
    #[serde(default)]
    struct Sample {
        size: u32,
        inner: Inner,
    }

    #[derive(Debug, Default, Deserialize, PartialEq)]
    #[serde(default)]
    struct Inner {
        on: bool,
    }

    #[test]
    fn clave_desconocida_avisa_con_posicion_y_aplica_el_resto() {
        let (value, warnings) =
            parse::<Sample>("size = 3\n[inner]\non = true\n  typo = 1\n").unwrap();
        assert_eq!(
            value,
            Sample {
                size: 3,
                inner: Inner { on: true }
            }
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, Severity::Warning);
        assert_eq!(
            warnings[0].message,
            "clave desconocida `inner.typo`, se ignora"
        );
        assert_eq!(warnings[0].location, Some(Location { line: 4, column: 3 }));
    }

    #[test]
    fn toml_roto_da_error_con_posicion() {
        let error = parse::<Sample>("size = 3\nsize = = 4\n").unwrap_err();
        assert_eq!(error.severity, Severity::Error);
        assert_eq!(error.location.map(|at| at.line), Some(2));
    }

    #[test]
    fn tipo_equivocado_da_error_con_posicion() {
        let error = parse::<Sample>("[inner]\non = \"sí\"\n").unwrap_err();
        assert_eq!(error.location, Some(Location { line: 2, column: 6 }));
        assert!(error.message.contains("bool"), "{}", error.message);
    }
}
