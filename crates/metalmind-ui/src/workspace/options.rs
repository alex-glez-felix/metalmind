//! `:set` sobre las opciones de un panel: hoy, sus números de línea.

use super::LineNumbers;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OptionError {
    #[error("E471: falta el argumento")]
    MissingArgument,
    #[error("E518: opción desconocida: {0}")]
    Unknown(String),
}

#[derive(Clone, Copy)]
enum Change {
    On,
    Off,
    Toggle,
}

/// Lo escrito tras `:set`, de izquierda a derecha como vim: `nu`, `nonu`,
/// `nu!` e `invnu`, con nombres largos o cortos. En la primera opción que no
/// conoce se detiene y deja aplicadas las anteriores.
///
/// # Errors
/// Sin argumento o con una opción desconocida.
pub fn set(numbers: &mut LineNumbers, argument: Option<&str>) -> Result<(), OptionError> {
    let mut words = argument.unwrap_or_default().split_whitespace().peekable();
    if words.peek().is_none() {
        return Err(OptionError::MissingArgument);
    }
    for word in words {
        let (flag, change) = parse(word).ok_or_else(|| OptionError::Unknown(word.to_owned()))?;
        let value = match flag {
            Flag::Number => &mut numbers.absolute,
            Flag::RelativeNumber => &mut numbers.relative,
        };
        *value = match change {
            Change::On => true,
            Change::Off => false,
            Change::Toggle => !*value,
        };
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Flag {
    Number,
    RelativeNumber,
}

fn parse(word: &str) -> Option<(Flag, Change)> {
    if let Some(name) = word.strip_suffix('!') {
        return Some((flag(name)?, Change::Toggle));
    }
    if let Some(flag) = flag(word) {
        return Some((flag, Change::On));
    }
    if let Some(name) = word.strip_prefix("inv") {
        return Some((flag(name)?, Change::Toggle));
    }
    Some((flag(word.strip_prefix("no")?)?, Change::Off))
}

fn flag(name: &str) -> Option<Flag> {
    match name {
        "number" | "nu" => Some(Flag::Number),
        "relativenumber" | "rnu" => Some(Flag::RelativeNumber),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn numbers(absolute: bool, relative: bool) -> LineNumbers {
        LineNumbers { absolute, relative }
    }

    #[test]
    fn cada_forma_de_set_como_vim() {
        let cases = [
            ("nu", numbers(false, false), numbers(true, false)),
            ("number", numbers(false, true), numbers(true, true)),
            ("nonu", numbers(true, true), numbers(false, true)),
            ("nonumber", numbers(true, false), numbers(false, false)),
            ("rnu", numbers(false, false), numbers(false, true)),
            ("relativenumber", numbers(true, false), numbers(true, true)),
            ("nornu", numbers(true, true), numbers(true, false)),
            ("nu!", numbers(true, true), numbers(false, true)),
            ("rnu!", numbers(true, false), numbers(true, true)),
            ("invnu", numbers(false, false), numbers(true, false)),
            ("nu rnu", numbers(false, false), numbers(true, true)),
            (
                "  nonu   nornu ",
                numbers(true, true),
                numbers(false, false),
            ),
        ];
        for (argument, before, expected) in cases {
            let mut actual = before;
            assert_eq!(set(&mut actual, Some(argument)), Ok(()), "{argument:?}");
            assert_eq!(actual, expected, "{argument:?} desde {before:?}");
        }
    }

    #[test]
    fn una_opcion_desconocida_deja_las_anteriores() {
        let mut actual = numbers(false, false);
        assert_eq!(
            set(&mut actual, Some("nu wrap rnu")),
            Err(OptionError::Unknown("wrap".to_owned()))
        );
        assert_eq!(actual, numbers(true, false));
        for word in ["no", "nonsense", "nu!!", "!", "invrnu!"] {
            assert_eq!(
                set(&mut actual, Some(word)),
                Err(OptionError::Unknown(word.to_owned())),
                "{word:?}"
            );
        }
    }

    #[test]
    fn sin_argumento_es_un_error() {
        let mut actual = LineNumbers::HYBRID;
        assert_eq!(set(&mut actual, None), Err(OptionError::MissingArgument));
        assert_eq!(
            set(&mut actual, Some("  ")),
            Err(OptionError::MissingArgument)
        );
    }
}
