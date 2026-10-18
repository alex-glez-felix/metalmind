//! `:s`: leer el comando y aplicarlo en un solo paso de undo.

mod replacement;

use std::ops::RangeInclusive;

use nota_buffer::{Buffer, Edit, Selection, Selections};

pub(crate) use replacement::Replacement;

use crate::lines::{first_non_blank, last_char};
use crate::search::{Case, Pattern, split_at_delimiter};
use crate::{CommandError, VimError};

/// Cuántos reemplazos hizo un `:s` y en cuántas líneas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Substitution {
    pub replacements: usize,
    pub lines: usize,
}

/// Desde cuántos reemplazos se avisa, como `report=2` de vim.
const REPORT: usize = 2;

impl Substitution {
    /// El aviso de vim (`3 substitutions on 2 lines`); `None` si fueron
    /// pocos para avisar.
    #[must_use]
    pub fn report(self) -> Option<String> {
        let plural = |n: usize| if n == 1 { "" } else { "s" };
        (self.replacements > REPORT).then(|| {
            format!(
                "{} substitution{} on {} line{}",
                self.replacements,
                plural(self.replacements),
                self.lines,
                plural(self.lines),
            )
        })
    }
}

/// Un `:s` tal como se escribió.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SubstituteCommand {
    /// `None` en un `:s` sin patrón, que repite el último.
    pub new: Option<NewSubstitute>,
    pub flags: Flags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NewSubstitute {
    /// Vacío reusa el último patrón usado, de búsqueda o de `:s`.
    pub pattern: String,
    pub replacement: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Flags {
    /// `g`: todas las coincidencias de cada línea, no solo la primera.
    pub global: bool,
    /// `i` o `I`.
    pub case: Option<Case>,
    /// `e`: no es error que el patrón no esté.
    pub quiet: bool,
}

impl SubstituteCommand {
    /// `args` es lo que sigue al nombre del comando.
    ///
    /// # Errors
    /// Con un flag que no se soporta o texto de más.
    pub fn parse(args: &str) -> Result<Self, CommandError> {
        let Some(delimiter) = args.chars().next().filter(|&c| is_delimiter(c)) else {
            return Ok(Self {
                new: None,
                flags: Flags::parse(args.trim_start())?,
            });
        };
        let (pattern, rest) = split_at_delimiter(&args[delimiter.len_utf8()..], delimiter);
        let (replacement, rest) = rest.map_or((String::new(), None), |rest| {
            split_at_delimiter(rest, delimiter)
        });
        Ok(Self {
            new: Some(NewSubstitute {
                pattern,
                replacement,
            }),
            flags: Flags::parse(rest.unwrap_or_default())?,
        })
    }
}

/// Como vim: cualquier carácter que no sea letra, número, espacio, `\`, `"`
/// ni `|`.
fn is_delimiter(c: char) -> bool {
    !(c.is_alphanumeric() || c.is_whitespace() || matches!(c, '\\' | '"' | '|'))
}

impl Flags {
    fn parse(text: &str) -> Result<Self, CommandError> {
        let mut flags = Self::default();
        for (at, c) in text.char_indices() {
            match c {
                // Como vim, otra `g` lo vuelve a apagar.
                'g' => flags.global = !flags.global,
                'i' => flags.case = Some(Case::Ignore),
                'I' => flags.case = Some(Case::Match),
                'e' => flags.quiet = true,
                'c' | 'n' | 'p' | 'l' | 'r' | '#' | '&' => {
                    return Err(CommandError::UnsupportedFlag(c));
                }
                _ if text[at..].trim().is_empty() => break,
                _ => {
                    return Err(CommandError::TrailingCharacters(
                        text[at..].trim().to_owned(),
                    ));
                }
            }
        }
        Ok(flags)
    }
}

/// Lo que dejó un `:s` que reemplazó algo.
pub(crate) struct Applied {
    pub summary: Substitution,
    pub selections: Selections,
}

/// Reemplaza en `lines` en una sola transacción; `None` si el patrón no está.
/// Como vim, el cursor queda en el primer no blanco de la última línea
/// cambiada, y el undo lo lleva al inicio de la primera.
pub(crate) fn apply(
    buffer: &mut Buffer,
    pattern: &Pattern,
    replacement: &Replacement,
    lines: RangeInclusive<usize>,
    global: bool,
) -> Result<Option<Applied>, VimError> {
    let mut edits = Vec::new();
    let mut summary = Substitution {
        replacements: 0,
        lines: 0,
    };
    let mut first_changed = None;
    let mut last_changed = 0;
    let mut added_lines = 0;
    for line in pattern.matching_lines(buffer, *lines.start()..lines.end() + 1) {
        let start = buffer.line_start(line);
        let text = buffer.line_text(line);
        let taken = if global { usize::MAX } else { 1 };
        let before = edits.len();
        for captures in pattern.line_captures(&text).take(taken) {
            let Some(found) = captures.get(0) else {
                continue;
            };
            let new = replacement.expand(&captures);
            added_lines += new.matches('\n').count();
            edits.push(Edit::replace(
                start + found.start()..start + found.end(),
                new,
            ));
        }
        if edits.len() > before {
            summary.replacements += edits.len() - before;
            summary.lines += 1;
            first_changed.get_or_insert(line);
            last_changed = line + added_lines;
        }
    }
    let Some(first_changed) = first_changed else {
        return Ok(None);
    };
    let undo_cursor = Selection::cursor(buffer.line_start(first_changed));
    buffer.begin_transaction(Selections::single(undo_cursor))?;
    buffer.apply(edits)?;
    let cursor = first_non_blank(buffer, last_changed).min(last_char(buffer, last_changed));
    let selections = Selections::single(Selection::cursor(cursor));
    buffer.commit_transaction(selections.clone())?;
    Ok(Some(Applied {
        summary,
        selections,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &str) -> Result<SubstituteCommand, CommandError> {
        SubstituteCommand::parse(args)
    }

    fn new(pattern: &str, replacement: &str) -> NewSubstitute {
        NewSubstitute {
            pattern: pattern.to_owned(),
            replacement: replacement.to_owned(),
        }
    }

    #[test]
    fn parses_pattern_replacement_and_flags() {
        let command = parse("/a/b/gI").unwrap();
        assert_eq!(command.new, Some(new("a", "b")));
        assert_eq!(
            command.flags,
            Flags {
                global: true,
                case: Some(Case::Match),
                quiet: false
            }
        );
        assert_eq!(parse("/a/b").unwrap().new, Some(new("a", "b")));
        assert_eq!(parse("/a").unwrap().new, Some(new("a", "")));
        assert_eq!(parse("#a/b#c\\#d#").unwrap().new, Some(new("a/b", "c\\#d")));
        assert_eq!(parse("/a\\/b/c/").unwrap().new, Some(new("a\\/b", "c")));
        assert!(!parse("/a/b/gg").unwrap().flags.global);
        assert!(parse("/a/b/e ").unwrap().flags.quiet);
        assert_eq!(
            parse("/a/b/ e"),
            Err(CommandError::TrailingCharacters("e".to_owned()))
        );
    }

    #[test]
    fn without_a_delimiter_repeats_the_last() {
        let command = parse(" g").unwrap();
        assert_eq!(command.new, None);
        assert!(command.flags.global);
        assert_eq!(parse("").unwrap().new, None);
    }

    #[test]
    fn rejects_unsupported_flags_and_leftovers() {
        assert_eq!(parse("/a/b/c"), Err(CommandError::UnsupportedFlag('c')));
        assert_eq!(parse("/a/b/gn"), Err(CommandError::UnsupportedFlag('n')));
        assert_eq!(
            parse("/a/b/g 3"),
            Err(CommandError::TrailingCharacters("3".to_owned()))
        );
        assert_eq!(
            parse("/a/b/x"),
            Err(CommandError::TrailingCharacters("x".to_owned()))
        );
    }
}
