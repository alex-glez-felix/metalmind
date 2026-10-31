//! Lectura de YAML 1.2 a tipos de serde. Una clave desconocida es un aviso y no
//! tumba el archivo: un typo no debe costar el resto de la config.
//!
//! Solo `true` y `false` son booleanos: `on`, `yes` o `no` en un campo
//! booleano son un error de tipo, y en uno de texto son ese texto.

mod keys;
mod message;

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer};

use crate::diagnostic::{Diagnostic, Location};

/// Un valor con dónde se escribió. Dos valores son iguales si lo son sus
/// contenidos, se hayan escrito donde se hayan escrito.
#[derive(Clone, Debug)]
pub(crate) struct Spanned<T> {
    value: T,
    at: Option<Location>,
}

impl<T> Spanned<T> {
    /// Un valor que no salió de ningún archivo.
    pub(crate) fn new(value: T) -> Self {
        Self { value, at: None }
    }

    pub(crate) fn get_ref(&self) -> &T {
        &self.value
    }

    pub(crate) fn into_inner(self) -> T {
        self.value
    }

    pub(crate) fn location(&self) -> Option<Location> {
        self.at
    }
}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Spanned<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let spanned = serde_saphyr::Spanned::<T>::deserialize(deserializer)?;
        Ok(Self {
            at: location(spanned.referenced),
            value: spanned.value,
        })
    }
}

/// El valor leído y los avisos por claves ignoradas.
///
/// # Errors
/// Si el YAML está roto o un valor no tiene el tipo esperado.
pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> Result<(T, Vec<Diagnostic>), Diagnostic> {
    let mut ignored = Vec::new();
    let value = serde_saphyr::with_deserializer_from_str_with_options(text, options(), |yaml| {
        serde_ignored::deserialize(yaml, |path| ignored.extend(keys::segments(&path)))
    })
    .map_err(|error| to_diagnostic(&error))?;
    // Solo con claves ignoradas hace falta una segunda pasada para ubicarlas.
    let tree = if ignored.is_empty() {
        None
    } else {
        serde_saphyr::from_str_with_options::<keys::Node>(text, options()).ok()
    };
    let warnings = ignored
        .into_iter()
        .map(|path| {
            Diagnostic::warning(format!("clave desconocida `{}`, se ignora", path.join(".")))
                .at(tree.as_ref().and_then(|tree| tree.key_location(&path)))
        })
        .collect();
    Ok((value, warnings))
}

pub(crate) fn options() -> serde_saphyr::Options {
    serde_saphyr::options! { strict_booleans: true }
}

pub(crate) fn to_diagnostic(error: &serde_saphyr::Error) -> Diagnostic {
    Diagnostic::error(message::render(error)).at(error.location().and_then(location))
}

fn location(at: serde_saphyr::Location) -> Option<Location> {
    if at == serde_saphyr::Location::UNKNOWN {
        return None;
    }
    Some(Location {
        line: usize::try_from(at.line()).ok()?,
        column: usize::try_from(at.column()).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde::Deserialize;

    use super::*;
    use crate::diagnostic::Severity;

    #[derive(Debug, Default, Deserialize, PartialEq)]
    #[serde(default)]
    struct Sample {
        size: u32,
        name: String,
        inner: Inner,
    }

    #[derive(Debug, Default, Deserialize, PartialEq)]
    #[serde(default)]
    struct Inner {
        on: bool,
        label: Option<Spanned<String>>,
    }

    #[test]
    fn clave_desconocida_avisa_con_posicion_y_aplica_el_resto() {
        let (value, warnings) =
            parse::<Sample>("size: 3\ninner:\n  on: true\n  typo: 1\n").unwrap();
        assert_eq!(
            value,
            Sample {
                size: 3,
                inner: Inner {
                    on: true,
                    label: None
                },
                ..Sample::default()
            }
        );
        let [warning] = warnings.as_slice() else {
            panic!("{warnings:?}")
        };
        assert_eq!(warning.severity, Severity::Warning);
        assert_eq!(warning.message, "clave desconocida `inner.typo`, se ignora");
        assert_eq!(warning.location, Some(Location { line: 4, column: 3 }));
    }

    #[test]
    fn una_clave_compleja_ignorada_no_borra_las_posiciones() {
        let (_, warnings) = parse::<Sample>("extra: { [a]: 1 }\ninner:\n  typo: 1\n").unwrap();
        let locations: Vec<_> = warnings.iter().map(|warning| warning.location).collect();
        assert_eq!(
            locations,
            [
                Some(Location { line: 1, column: 1 }),
                Some(Location { line: 3, column: 3 })
            ]
        );
    }

    #[test]
    fn un_valor_con_posicion_no_es_una_clave_desconocida() {
        let (value, warnings) = parse::<Sample>("inner:\n  label: ñandú\n").unwrap();
        assert_eq!(warnings, []);
        let label = value.inner.label.unwrap();
        assert_eq!(label.get_ref(), "ñandú");
        assert_eq!(
            label.location(),
            Some(Location {
                line: 2,
                column: 10
            })
        );
    }

    #[test]
    fn yaml_roto_da_error_con_linea_y_columna() {
        let error = parse::<Sample>("size: 3\nname: [a\n").unwrap_err();
        assert_eq!(error.severity, Severity::Error);
        assert_eq!(error.location.map(|at| at.line), Some(2));
    }

    #[test]
    fn indentacion_equivocada_da_error_con_linea_y_columna() {
        let error = parse::<Sample>("inner:\n  on: true\n label: x\n").unwrap_err();
        assert_eq!(error.location, Some(Location { line: 3, column: 2 }));
        let error = parse::<Sample>("name: a\n  size: 3\n").unwrap_err();
        assert_eq!(error.location, Some(Location { line: 2, column: 7 }));
    }

    #[test]
    fn el_mensaje_no_repite_la_posicion() {
        let error = parse::<Sample>("name: a\n  size: 3\n").unwrap_err();
        assert!(!error.message.contains("line"), "{}", error.message);
        assert!(!error.message.contains('\n'), "{}", error.message);
    }

    #[test]
    fn tipo_equivocado_da_error_con_posicion() {
        let error = parse::<Sample>("inner:\n  on: \"sí\"\n").unwrap_err();
        assert_eq!(error.location, Some(Location { line: 2, column: 7 }));
        assert!(error.message.contains("boolean"), "{}", error.message);
    }

    #[test]
    fn una_clave_repetida_es_error() {
        let error = parse::<Sample>("size: 1\nsize: 2\n").unwrap_err();
        assert_eq!(error.location, Some(Location { line: 2, column: 1 }));
    }

    /// El problema de Noruega: en YAML 1.1, `NO`, `on`, `yes` y `off` son
    /// booleanos aunque el campo espere texto.
    #[test]
    fn yes_no_on_off_nunca_son_booleanos_silenciosos() {
        for word in ["NO", "no", "on", "yes", "off", "y", "n", "Yes", "OFF"] {
            let (value, _) = parse::<Sample>(&format!("name: {word}\n")).unwrap();
            assert_eq!(value.name, word);
            let error = parse::<Sample>(&format!("inner:\n  on: {word}\n")).unwrap_err();
            assert_eq!(error.severity, Severity::Error, "{word}");
            assert_eq!(error.location, Some(Location { line: 2, column: 7 }));
            assert!(error.message.contains("true or false"), "{}", error.message);
        }
    }

    #[test]
    fn json_puro_es_yaml() {
        let yaml = parse::<Sample>("size: 3\ninner:\n  on: true\n  label: x\n").unwrap();
        let json = parse::<Sample>(r#"{"size": 3, "inner": {"on": true, "label": "x"}}"#).unwrap();
        assert_eq!(json, yaml);
    }

    #[test]
    fn vacio_o_solo_comentarios_es_todo_por_defecto() {
        for text in ["", "\n", "# nada\n"] {
            assert_eq!(parse::<Sample>(text).unwrap(), (Sample::default(), vec![]));
        }
    }
}
