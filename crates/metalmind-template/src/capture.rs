//! `metalmind add` con plantilla (06 §11): la nota capturada nace de la
//! plantilla pedida o de la que pida la página de uno de sus tags, con el
//! texto capturado donde la UI dejaría el cursor. Lee solo las carpetas de
//! plantillas y de tags, no el vault entero.

use jiff::Zoned;
use metalmind_vault::tags::Tags;
use metalmind_vault::{Capture, CaptureError, Captured, InvalidPath, VaultPath, VaultRoot, read};

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
/// la que pidan las páginas de sus tags; si ninguna pide una, o piden
/// distintas, es la captura de siempre. `{{title}}` es el título o, sin él,
/// el nombre del archivo.
///
/// # Errors
/// Los de la captura, una plantilla que no existe o un texto con
/// frontmatter cuando se usa una plantilla.
pub fn capture(
    root: &VaultRoot,
    note: &metalmind_vault::NewNote,
    pick: Pick<'_>,
    folders: Folders<'_>,
    now: &Zoned,
) -> Result<FromTemplate, AddError> {
    let plain = |problems| -> Result<FromTemplate, AddError> {
        Ok(FromTemplate {
            captured: root.capture(note, now.datetime())?,
            template: None,
            problems,
        })
    };
    if pick == Pick::Blank || (pick == Pick::ByTags && note.tags.is_empty()) {
        return plain(Vec::new());
    }
    let templates_folder = templates_folder(folders.templates)?;
    let index = root.files_within(&[templates_folder.clone(), folders.tags.clone()]);
    let tags = Tags::build(folders.tags.clone(), &index, |page: &VaultPath| {
        read(&root.path().join(page.as_str())).ok().flatten()
    });
    let graph = tags.graph();
    let loaded = Templates::load(root.path(), &index, &templates_folder);
    let mut problems: Vec<String> = loaded.errors.iter().map(ToString::to_string).collect();
    let templates = loaded.value;
    let mut note = note.clone();
    match pick {
        Pick::Named(name) if templates.get(name).is_none() => {
            return Err(AddError::MissingTemplate {
                name: name.to_owned(),
                folder: templates_folder,
            });
        }
        Pick::Named(name) => note
            .tags
            .extend(tag_of_template(graph, name).map(str::to_owned)),
        _ => match tag_template(graph, &note.tags) {
            TagChoice::Template { .. } => {}
            TagChoice::None => return plain(problems),
            TagChoice::Conflict(asked) => {
                let asked = asked
                    .into_iter()
                    .map(|(tag, template)| (tag.to_owned(), template.to_owned()))
                    .collect();
                problems.push(DraftProblem::Conflict { asked }.to_string());
                return plain(problems);
            }
        },
    }
    if metalmind_syntax::scan(&note.text).frontmatter.is_some() {
        return Err(AddError::FrontmatterWithTemplate);
    }
    let mut template = None;
    let captured = root.capture_with(&note, now.datetime(), |capture| {
        let filled = fill(capture, pick, &templates, &tags, now);
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

fn fill(
    capture: Capture<'_>,
    pick: Pick<'_>,
    templates: &Templates,
    tags: &Tags,
    now: &Zoned,
) -> crate::Draft {
    let note = NewNote {
        title: capture.title.unwrap_or(capture.stem),
        tags: capture.tags,
        pick,
        now,
    };
    draft(&note, templates, tags.graph())
}

/// `text` con `captured` en `cursor`, en su propia línea; sin dejar líneas
/// en blanco de más al final.
fn with_text(text: &str, cursor: usize, captured: &str) -> String {
    let captured = captured.trim_end_matches(['\n', '\r']);
    let (before, mut after) = text.split_at(cursor.min(text.len()));
    let mut out = String::with_capacity(text.len() + captured.len() + 2);
    out.push_str(before);
    if !captured.trim().is_empty() {
        // Al final de una línea, el salto que la cierra pasa a cerrar lo capturado.
        if !before.is_empty() && !before.ends_with('\n') {
            out.push('\n');
            after = after.strip_prefix('\n').unwrap_or(after);
        }
        out.push_str(captured);
        out.push('\n');
    }
    out.push_str(after);
    let end = out.trim_end_matches(['\n', '\r']).len();
    out.truncate(end);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests;
