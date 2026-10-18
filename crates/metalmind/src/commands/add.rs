//! `metalmind add`: captura una nota en la raíz del vault.

use std::borrow::Cow;
use std::io::{self, IsTerminal as _, Read as _};

use jiff::Zoned;
use metalmind_vault::{Captured, NewNote};
use serde::Serialize;

use super::{CliError, print, root_text, vault_root};
use crate::cli::AddArgs;

#[derive(Serialize)]
struct Added<'a> {
    vault: Cow<'a, str>,
    #[serde(flatten)]
    note: &'a Captured,
}

/// Crea la nota e imprime su ruta relativa al vault.
///
/// # Errors
/// Si no hay texto ni título, algún tag no vale, no hay vault o falla la escritura.
pub fn add(args: AddArgs) -> Result<(), CliError> {
    let vault = vault_root(&args.common)?;
    let text = text(&args.text, args.title.is_some())?;
    let note = NewNote {
        text,
        title: args.title,
        tags: args.tags,
    };
    let captured = vault.capture(&note, Zoned::now().datetime())?;
    let added = Added {
        vault: root_text(vault.path()),
        note: &captured,
    };
    print(args.common.json, &added, |out| {
        writeln!(out, "{}", captured.path)
    })
}

/// Las palabras del argumento o, si no hay, stdin. Una terminal no cuenta
/// como stdin: esperar ahí a que alguien escriba parecería un cuelgue.
fn text(words: &[String], has_title: bool) -> Result<String, CliError> {
    if !words.is_empty() {
        return Ok(words.join(" "));
    }
    let mut stdin = io::stdin().lock();
    if stdin.is_terminal() {
        return if has_title {
            Ok(String::new())
        } else {
            Err(CliError::Usage(
                "falta el texto: pásalo como argumento o por stdin".to_owned(),
            ))
        };
    }
    let mut text = String::new();
    stdin.read_to_string(&mut text).map_err(CliError::Stdin)?;
    Ok(text)
}
