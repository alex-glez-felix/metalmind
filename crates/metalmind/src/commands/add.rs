//! `metalmind add`: captura una nota en la raíz del vault, desde una
//! plantilla si se pide o si la pide la página de uno de sus tags (06 §11).

use std::borrow::Cow;
use std::io::{self, IsTerminal as _, Read as _, Write as _};

use jiff::Zoned;
use metalmind_template::{Folders, FromTemplate, Pick, capture};
use metalmind_vault::{Captured, NewNote, VaultOptions};
use serde::Serialize;

use super::{CliError, Config, print, root_text};
use crate::cli::AddArgs;

#[derive(Serialize)]
struct Added<'a> {
    vault: Cow<'a, str>,
    #[serde(flatten)]
    note: &'a Captured,
    template: Option<&'a str>,
    problems: &'a [String],
}

/// Crea la nota e imprime su ruta relativa al vault; lo que salió distinto
/// de lo pedido (una variable mal escrita en la plantilla) va a stderr.
///
/// # Errors
/// Si no hay texto ni título ni plantilla, algún tag no vale, la plantilla
/// no existe, no hay vault o falla la escritura.
pub fn add(args: AddArgs) -> Result<(), CliError> {
    let config = Config::load();
    let vault = config.vault_root(&args.common)?;
    let pick = match (&args.template, args.no_template) {
        (Some(name), _) => Pick::Named(name),
        (None, true) => Pick::Blank,
        (None, false) => Pick::ByTags,
    };
    let text = text(&args.text, may_be_empty(&args))?;
    let note = NewNote {
        text,
        title: args.title,
        tags: args.tags,
    };
    let tags_folder = VaultOptions::default().tags_folder;
    let folders = Folders {
        templates: &config.settings.templates_folder,
        tags: &tags_folder,
    };
    let FromTemplate {
        captured,
        template,
        problems,
    } = capture(&vault, &note, pick, folders, &Zoned::now())?;
    if !args.common.json {
        let mut stderr = io::stderr().lock();
        for problem in &problems {
            // Si stderr no acepta el aviso, la nota ya está creada igual.
            let _ = writeln!(stderr, "metalmind: {}: {problem}", captured.path);
        }
    }
    let added = Added {
        vault: root_text(vault.path()),
        note: &captured,
        template: template.as_deref(),
        problems: &problems,
    };
    print(args.common.json, &added, |out| {
        writeln!(out, "{}", captured.path)
    })
}

/// Si la nota puede nacer sin texto: con título, o con una plantilla
/// elegida o que pueda pedir un tag. Si al final no hay plantilla, la
/// captura sin texto ni título es error igual.
fn may_be_empty(args: &AddArgs) -> bool {
    args.title.is_some() || args.template.is_some() || (!args.tags.is_empty() && !args.no_template)
}

/// Las palabras del argumento o, si no hay, stdin. Una terminal no cuenta
/// como stdin: esperar ahí a que alguien escriba parecería un cuelgue.
fn text(words: &[String], optional: bool) -> Result<String, CliError> {
    if !words.is_empty() {
        return Ok(words.join(" "));
    }
    let mut stdin = io::stdin().lock();
    if stdin.is_terminal() {
        return if optional {
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

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;
    use crate::cli::{Cli, Command};

    fn may_be_empty_with(args: &[&str]) -> bool {
        let cli = Cli::try_parse_from(["metalmind", "add"].iter().chain(args)).unwrap();
        let Some(Command::Add(args)) = cli.command else {
            panic!("{cli:?}");
        };
        may_be_empty(&args)
    }

    /// Con stdin en una terminal no se espera texto si la nota puede nacer
    /// sin él: `metalmind add -t reunion` crea la plantilla de la reunión.
    #[test]
    fn sin_texto_vale_si_puede_haber_titulo_o_plantilla() {
        assert!(!may_be_empty_with(&[]));
        assert!(may_be_empty_with(&["--title", "Plan"]));
        assert!(may_be_empty_with(&["--template", "semanal"]));
        assert!(may_be_empty_with(&["-t", "reunion"]));
        assert!(!may_be_empty_with(&["-t", "reunion", "--no-template"]));
    }
}
