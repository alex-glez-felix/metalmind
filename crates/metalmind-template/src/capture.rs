//! `metalmind add` con plantilla (06 §11): la nota capturada nace de la
//! plantilla pedida o de la que pida la página de uno de sus tags, con el
//! texto capturado donde la UI dejaría el cursor. Lee solo las carpetas de
//! plantillas y de tags, no el vault entero.

use std::sync::{Mutex, PoisonError};

use jiff::Zoned;
use metalmind_vault::tags::{TagGraph, Tags};
use metalmind_vault::{CaptureError, Captured, InvalidPath, VaultPath, VaultRoot, Within, read};

use crate::draft::{DraftProblem, NewNote, Pick, Templates, draft};
use crate::load::templates_folder;
use crate::tags::{TagChoice, tag_of_template, tag_template};

/// Dónde están las plantillas y las páginas de tag del vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Folders<'a> {
    /// Como en `templates_folder` de la config: se lee solo si hace falta,
    /// así una que no vale no impide una captura sin plantilla.
    pub templates: &'a str,
    pub tags: &'a VaultPath,
}

/// La nota creada y cómo salió.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromTemplate {
    pub captured: Captured,
    /// La plantilla que se usó, si se usó una.
    pub template: Option<String>,
    /// Lo que salió distinto de lo pedido; la nota se creó igual.
    pub problems: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error("no existe la plantilla `{name}` en {folder}/")]
    MissingTemplate { name: String, folder: VaultPath },
    #[error("`templates_folder` de config.yaml: {0}")]
    TemplatesFolder(#[from] InvalidPath),
    #[error("el texto ya trae frontmatter: con una plantilla, los tags van con -t")]
    FrontmatterWithTemplate,
}

/// Crea `note` en la raíz del vault. Con `Pick::Named`, desde esa plantilla
/// y con el tag cuya página la pide, como en la UI. Con `Pick::ByTags`, desde
/// la que pidan las páginas de sus tags; si ninguna pide una, piden
/// distintas o la pedida no existe, es la captura de siempre, con el aviso.
/// `{{title}}` es el título o, sin él, el nombre del archivo.
///
/// # Errors
/// Los de la captura, una plantilla elegida que no existe, una carpeta de
/// plantillas que no vale cuando se usa una, o un texto con frontmatter
/// cuando se usa una plantilla.
pub fn capture(
    root: &VaultRoot,
    note: &metalmind_vault::NewNote,
    pick: Pick<'_>,
    folders: Folders<'_>,
    now: &Zoned,
) -> Result<FromTemplate, AddError> {
    if pick == Pick::Blank || (pick == Pick::ByTags && note.tags.is_empty()) {
        return plain(root, note, now, Vec::new());
    }
    // La carpeta de plantillas de la config solo importa si se usa una.
    let templates_folder = templates_folder(folders.templates);
    // Solo las páginas de tag: el grafo sale de ellas. Una carpeta o una
    // página que no se puede leer no pide plantilla: se avisa.
    let pages = root.files_within(std::slice::from_ref(folders.tags));
    let unread = Mutex::new(unreadable(&pages));
    let tags = Tags::build(folders.tags.clone(), &pages.index, |page: &VaultPath| {
        read(&root.path().join(page.as_str()))
            .map_err(|error| {
                let mut unread = unread.lock().unwrap_or_else(PoisonError::into_inner);
                unread.push(error.to_string());
            })
            .ok()
            .flatten()
    });
    let mut problems = unread.into_inner().unwrap_or_else(PoisonError::into_inner);
    let chosen = match choose(pick, tags.graph(), &note.tags) {
        Ok(chosen) => chosen,
        Err(why) => {
            problems.extend(why);
            return plain(root, note, now, problems);
        }
    };
    let templates_folder = templates_folder?;
    let within = root.files_within(std::slice::from_ref(&templates_folder));
    problems.extend(unreadable(&within));
    let loaded = Templates::load(root.path(), &within.index, &templates_folder);
    problems.extend(loaded.errors.iter().map(ToString::to_string));
    if loaded.value.get(&chosen.name).is_some() {
        return from_template(root, note, &chosen, &loaded.value, &tags, now, problems);
    }
    let Some(tag) = chosen.asked_by else {
        return Err(AddError::MissingTemplate {
            name: chosen.name,
            folder: templates_folder,
        });
    };
    problems.push(
        DraftProblem::MissingTemplate {
            name: chosen.name,
            tag: Some(tag),
        }
        .to_string(),
    );
    plain(root, note, now, problems)
}

fn unreadable(within: &Within) -> Vec<String> {
    within
        .unreadable
        .iter()
        .map(|folder| format!("no se pudo leer {folder}/"))
        .collect()
}

/// La plantilla que se usa: la elegida, o la que pide la página de un tag.
struct Chosen<'p> {
    pick: Pick<'p>,
    name: String,
    /// El tag cuya página la pide, si no se eligió a mano.
    asked_by: Option<String>,
    /// El tag que la elegida trae consigo, como en la UI.
    brings: Option<String>,
}

/// Sin plantilla, los avisos de por qué.
fn choose<'p>(
    pick: Pick<'p>,
    graph: &TagGraph,
    tags: &[String],
) -> Result<Chosen<'p>, Vec<String>> {
    if let Pick::Named(name) = pick {
        return Ok(Chosen {
            pick,
            name: name.to_owned(),
            asked_by: None,
            brings: tag_of_template(graph, name).map(str::to_owned),
        });
    }
    match tag_template(graph, tags) {
        TagChoice::Template { template, tag } => Ok(Chosen {
            pick,
            name: template.to_owned(),
            asked_by: Some(tag.to_owned()),
            brings: None,
        }),
        TagChoice::None => Err(Vec::new()),
        TagChoice::Conflict(asked) => {
            let asked = asked
                .into_iter()
                .map(|(tag, template)| (tag.to_owned(), template.to_owned()))
                .collect();
            Err(vec![DraftProblem::Conflict { asked }.to_string()])
        }
    }
}

/// La captura de siempre, con los avisos de por qué no hubo plantilla.
fn plain(
    root: &VaultRoot,
    note: &metalmind_vault::NewNote,
    now: &Zoned,
    problems: Vec<String>,
) -> Result<FromTemplate, AddError> {
    Ok(FromTemplate {
        captured: root.capture(note, now.datetime())?,
        template: None,
        problems,
    })
}

fn from_template(
    root: &VaultRoot,
    note: &metalmind_vault::NewNote,
    chosen: &Chosen<'_>,
    templates: &Templates,
    tags: &Tags,
    now: &Zoned,
    mut problems: Vec<String>,
) -> Result<FromTemplate, AddError> {
    if metalmind_syntax::scan(&note.text).frontmatter.is_some() {
        return Err(AddError::FrontmatterWithTemplate);
    }
    let mut note = note.clone();
    note.tags.extend(chosen.brings.clone());
    let mut template = None;
    let captured = root.capture_with(&note, now.datetime(), |capture| {
        let new = NewNote {
            title: capture.title.unwrap_or(capture.stem),
            tags: capture.tags,
            pick: chosen.pick,
            now,
        };
        let filled = draft(&new, templates, tags.graph());
        problems.extend(filled.problems.iter().map(ToString::to_string));
        template = filled.template;
        with_text(&filled.text, filled.cursor, capture.text)
    })?;
    Ok(FromTemplate {
        captured,
        template,
        problems,
    })
}

/// `text` con `captured` en `cursor`, en su propia línea y con los finales
/// de línea de la plantilla; sin dejar líneas en blanco de más al final.
fn with_text(text: &str, cursor: usize, captured: &str) -> String {
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let captured = captured.trim_end_matches(['\n', '\r']);
    let (before, mut after) = text.split_at(cursor.min(text.len()));
    let mut out = String::with_capacity(text.len() + captured.len() + 2 * ending.len());
    out.push_str(before);
    if !captured.trim().is_empty() {
        // Al final de una línea, el salto que la cierra pasa a cerrar lo capturado.
        if !before.is_empty() && !before.ends_with('\n') {
            out.push_str(ending);
            after = after.strip_prefix(ending).unwrap_or(after);
        }
        out.push_str(captured);
        out.push_str(ending);
    }
    out.push_str(after);
    let end = out.trim_end_matches(['\n', '\r']).len();
    out.truncate(end);
    out.push_str(ending);
    out
}

#[cfg(test)]
mod tests;
