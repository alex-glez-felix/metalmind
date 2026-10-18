//! El texto inicial de una nota nueva: su plantilla con las variables
//! llenas y sus tags en el frontmatter.

use std::collections::BTreeMap;

use jiff::Zoned;

use crate::render::{Context, VariableProblem, render};
use nota_vault::tags::TagGraph;

use crate::tags::{TagChoice, tag_template};
use crate::tags_field::{TagsNotAdded, with_tags};

/// Las plantillas del vault por nombre: su ruta dentro de la carpeta de
/// plantillas, sin `.md` (`reunion`, `trabajo/semanal`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Templates(BTreeMap<String, String>);

impl Templates {
    pub fn new(templates: impl IntoIterator<Item = (String, String)>) -> Self {
        Self(templates.into_iter().collect())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    /// En orden de bytes: lo que ofrece un selector de plantilla.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

/// Qué plantilla usar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick<'a> {
    /// La que pidan los tags de la nota, si alguno pide una.
    ByTags,
    Named(&'a str),
    /// Ninguna, aunque los tags pidan una.
    Blank,
}

#[derive(Debug, Clone, Copy)]
pub struct NewNote<'a> {
    pub title: &'a str,
    /// Los tags con los que nace: los de `-t` en la CLI, o los incluidos del
    /// focus (05 §3).
    pub tags: &'a [String],
    pub pick: Pick<'a>,
    pub now: &'a Zoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    /// La plantilla que se usó.
    pub template: Option<String>,
    pub problems: Vec<DraftProblem>,
}

/// Lo que salió distinto de lo pedido. La nota se puede crear igual.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DraftProblem {
    #[error("no existe la plantilla `{name}`")]
    MissingTemplate {
        name: String,
        /// El tag cuya página la pide, si no se eligió a mano.
        tag: Option<String>,
    },
    #[error("los tags piden plantillas distintas; no se usa ninguna")]
    Conflict {
        /// `(tag, plantilla)`.
        asked: Vec<(String, String)>,
    },
    #[error("en la plantilla: {0}")]
    Variable(VariableProblem),
    #[error("no se agregaron los tags: {0}")]
    TagsNotAdded(TagsNotAdded),
}

/// El texto inicial de `note`. Es puro: no lee ni escribe nada.
#[must_use]
pub fn draft(note: &NewNote, templates: &Templates, tags: &TagGraph) -> Draft {
    let mut problems = Vec::new();
    let template = pick(note, templates, tags, &mut problems);
    let body = match template {
        Some((_, text)) => {
            let rendered = render(
                text,
                Context {
                    title: note.title,
                    now: note.now,
                },
            );
            problems.extend(rendered.problems.into_iter().map(DraftProblem::Variable));
            rendered.text
        }
        None => String::new(),
    };
    let text = with_tags(&body, note.tags).unwrap_or_else(|error| {
        problems.push(DraftProblem::TagsNotAdded(error));
        body
    });
    Draft {
        text,
        template: template.map(|(name, _)| name.to_owned()),
        problems,
    }
}

fn pick<'t>(
    note: &NewNote,
    templates: &'t Templates,
    tags: &TagGraph,
    problems: &mut Vec<DraftProblem>,
) -> Option<(&'t str, &'t str)> {
    let (name, tag) = match note.pick {
        Pick::Blank => return None,
        Pick::Named(name) => (name, None),
        Pick::ByTags => match tag_template(tags, note.tags) {
            TagChoice::None => return None,
            TagChoice::Template { template, tag } => (template, Some(tag)),
            TagChoice::Conflict(asked) => {
                problems.push(DraftProblem::Conflict {
                    asked: asked
                        .into_iter()
                        .map(|(tag, template)| (tag.to_owned(), template.to_owned()))
                        .collect(),
                });
                return None;
            }
        },
    };
    let found = templates.0.get_key_value(name);
    if found.is_none() {
        problems.push(DraftProblem::MissingTemplate {
            name: name.to_owned(),
            tag: tag.map(str::to_owned),
        });
    }
    found.map(|(name, text)| (name.as_str(), text.as_str()))
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use jiff::tz::TimeZone;
    use pretty_assertions::assert_eq;

    use super::*;

    fn now() -> Zoned {
        let zone = TimeZone::posix("CST6").unwrap();
        "2026-10-18T21:07:09Z"
            .parse::<Timestamp>()
            .unwrap()
            .to_zoned(zone)
    }

    fn templates() -> Templates {
        Templates::new([
            (
                "reunion".to_owned(),
                "---\ntags: [reunion]\n---\n# {{title}} ({{date}})\n".to_owned(),
            ),
            ("diario".to_owned(), "# {{date:dddd D}}\n".to_owned()),
            ("rota".to_owned(), "{{autor}}".to_owned()),
        ])
    }

    fn pages() -> TagGraph {
        crate::test_vault::graph(&[
            ("tags/reunion.md", "---\ntemplate: reunion\n---\n"),
            ("tags/diario.md", "---\ntemplate: diario\n---\n"),
            ("tags/perdido.md", "---\ntemplate: no-existe\n---\n"),
        ])
    }

    fn drafted(tags: &[&str], pick: Pick) -> Draft {
        let tags: Vec<String> = tags.iter().map(|&tag| tag.to_owned()).collect();
        let note = NewNote {
            title: "Plan",
            tags: &tags,
            pick,
            now: &now(),
        };
        draft(&note, &templates(), &pages())
    }

    #[test]
    fn a_tag_page_picks_the_template_and_the_tags_are_merged() {
        let draft = drafted(&["reunion", "trabajo"], Pick::ByTags);
        assert_eq!(draft.problems, []);
        assert_eq!(draft.template.as_deref(), Some("reunion"));
        assert_eq!(
            draft.text,
            "---\ntags: [reunion, trabajo]\n---\n# Plan (2026-10-18)\n"
        );
    }

    #[test]
    fn without_tags_or_template_the_note_is_empty() {
        let draft = drafted(&[], Pick::ByTags);
        assert_eq!(draft.text, "");
        assert_eq!(draft.template, None);
        assert_eq!(draft.problems, []);
    }

    #[test]
    fn a_named_template_wins_over_the_tags() {
        let draft = drafted(&["reunion"], Pick::Named("diario"));
        assert_eq!(draft.text, "---\ntags: [reunion]\n---\n# domingo 18\n");
    }

    #[test]
    fn blank_ignores_the_tags_template_but_keeps_the_tags() {
        let draft = drafted(&["reunion"], Pick::Blank);
        assert_eq!(draft.text, "---\ntags: [reunion]\n---\n");
        assert_eq!(draft.template, None);
    }

    #[test]
    fn a_missing_named_template_is_reported() {
        let draft = drafted(&[], Pick::Named("nada"));
        assert_eq!(draft.text, "");
        assert_eq!(
            draft.problems,
            [DraftProblem::MissingTemplate {
                name: "nada".to_owned(),
                tag: None
            }]
        );
    }

    #[test]
    fn a_tag_page_naming_a_missing_template_is_reported_with_its_tag() {
        let draft = drafted(&["perdido"], Pick::ByTags);
        assert_eq!(draft.text, "---\ntags: [perdido]\n---\n");
        assert_eq!(
            draft.problems,
            [DraftProblem::MissingTemplate {
                name: "no-existe".to_owned(),
                tag: Some("perdido".to_owned())
            }]
        );
    }

    #[test]
    fn conflicting_tags_use_no_template() {
        let draft = drafted(&["reunion", "diario"], Pick::ByTags);
        assert_eq!(draft.template, None);
        assert_eq!(draft.text, "---\ntags: [reunion, diario]\n---\n");
        assert_eq!(
            draft.problems,
            [DraftProblem::Conflict {
                asked: vec![
                    ("reunion".to_owned(), "reunion".to_owned()),
                    ("diario".to_owned(), "diario".to_owned()),
                ]
            }]
        );
    }

    #[test]
    fn variable_problems_come_through() {
        let draft = drafted(&[], Pick::Named("rota"));
        assert_eq!(draft.text, "{{autor}}");
        assert_eq!(
            draft.problems,
            [DraftProblem::Variable(VariableProblem::Unknown {
                span: 0..9,
                text: "{{autor}}".to_owned()
            })]
        );
    }

    #[test]
    fn invalid_tags_leave_the_template_as_rendered() {
        let draft = drafted(&["con espacio"], Pick::Named("diario"));
        assert_eq!(draft.text, "# domingo 18\n");
        assert_eq!(
            draft.problems,
            [DraftProblem::TagsNotAdded(TagsNotAdded::InvalidTag(
                "con espacio".to_owned()
            ))]
        );
    }

    #[test]
    fn templates_are_listed_in_order() {
        let templates = templates();
        let names: Vec<_> = templates.names().collect();
        assert_eq!(names, ["diario", "reunion", "rota"]);
    }
}
