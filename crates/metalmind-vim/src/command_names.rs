//! Los nombres de los comandos de `:` que empiezan con lo escrito, del
//! motor y de quien aloja el editor, para autocompletarlos.

use crate::ActionName;
use crate::command::{HostCommand, abbreviates, split_bang};
use crate::ex::engine_names;

/// Un comando que se puede escribir tras `:`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandName {
    /// El nombre completo, con su `!` si lo lleva: `write!`.
    pub name: &'static str,
    /// La acción de un comando ajeno; `None` en los del motor.
    pub action: Option<ActionName>,
    takes_argument: bool,
}

impl CommandName {
    /// Lo que escribe aceptarlo. Tras un comando ajeno que lleva argumento
    /// va el espacio, así su argumento ya se autocompleta.
    #[must_use]
    pub fn completion(&self) -> String {
        if self.takes_argument {
            format!("{} ", self.name)
        } else {
            self.name.to_owned()
        }
    }
}

/// Los comandos cuyo nombre empieza con `typed`, unas letras con su `!` si
/// lo lleva: primero el que correría lo escrito, como lo abrevia vim, y los
/// demás en orden alfabético. Los nombres con `!` solo salen si se escribió
/// el `!`. Vacío si `typed` ya es el único nombre posible.
pub(crate) fn names_starting_with(typed: &str, commands: &[HostCommand]) -> Vec<CommandName> {
    let (letters, bang) = split_bang(typed);
    let engine = engine_names()
        .filter(|_| !bang)
        .filter(|(name, _)| name.starts_with(letters))
        .map(|(name, min)| {
            let runs = letters.len() >= min;
            (
                runs,
                CommandName {
                    name,
                    action: None,
                    takes_argument: false,
                },
            )
        });
    let host = commands
        .iter()
        .filter(|command| {
            let (name, name_bang) = split_bang(command.name);
            name_bang == bang && name.starts_with(letters)
        })
        .map(|command| {
            let runs = abbreviates(typed, command.name, command.min);
            (
                runs,
                CommandName {
                    name: command.name,
                    action: Some(command.action),
                    takes_argument: command.takes_argument,
                },
            )
        });
    let mut names: Vec<(bool, CommandName)> = engine.chain(host).collect();
    // El motor va antes que los ajenos, como al correr la línea: el primero
    // que corre es el que vale.
    let runs = names.iter().position(|(runs, _)| *runs);
    let first = runs.map(|index| names.remove(index).1);
    let mut names: Vec<CommandName> = names.into_iter().map(|(_, name)| name).collect();
    names.sort_by_key(|name| name.name);
    if let Some(first) = first {
        names.insert(0, first);
    }
    if let [only] = names.as_slice()
        && only.name == typed
    {
        names.clear();
    }
    names
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::RangeRule;

    const fn host(name: &'static str, min: usize, takes_argument: bool) -> HostCommand {
        HostCommand {
            name,
            min,
            action: ActionName::new("host::action"),
            takes_argument,
            with_range: RangeRule::NotAllowed,
        }
    }

    const COMMANDS: &[HostCommand] = &[
        host("write", 1, true),
        host("write!", 1, true),
        host("wall", 2, false),
        host("split", 2, false),
        host("focus", 5, true),
    ];

    fn names(typed: &str) -> Vec<&'static str> {
        names_starting_with(typed, COMMANDS)
            .into_iter()
            .map(|name| name.name)
            .collect()
    }

    #[test]
    fn primero_el_que_corre_y_luego_en_orden() {
        let cases: &[(&str, &[&str])] = &[
            // `:s` es `:substitute`, aunque `sort` y `split` empiecen igual.
            ("s", &["substitute", "sort", "split"]),
            ("so", &["sort"]),
            ("sp", &["split"]),
            // `:w` es `:write`; `:wall` pide dos letras.
            ("w", &["write", "wall"]),
            ("wa", &["wall"]),
            // `:fo` no corre nada: `focus` pide las cinco letras.
            ("fo", &["focus"]),
            ("t", &["t", "tableformat"]),
            ("zz", &[]),
        ];
        for &(typed, expected) in cases {
            assert_eq!(names(typed), expected, "{typed}");
        }
    }

    #[test]
    fn los_de_bang_solo_tras_el_bang() {
        assert_eq!(names("wri"), ["write"]);
        assert_eq!(names("w!"), ["write!"]);
    }

    #[test]
    fn nada_si_ya_es_el_unico_nombre() {
        assert_eq!(names("write"), Vec::<&str>::new());
        assert_eq!(names("wall"), Vec::<&str>::new());
        assert_eq!(names("t"), ["t", "tableformat"]);
    }

    #[test]
    fn aceptar_un_ajeno_con_argumento_deja_el_espacio() {
        let completions: Vec<String> = names_starting_with("f", COMMANDS)
            .iter()
            .map(CommandName::completion)
            .collect();
        assert_eq!(completions, ["focus "]);
        let completions: Vec<String> = names_starting_with("wa", COMMANDS)
            .iter()
            .map(CommandName::completion)
            .collect();
        assert_eq!(completions, ["wall"]);
    }
}
