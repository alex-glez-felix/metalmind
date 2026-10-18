//! La tabla de casos del `TagGraph` de 05 §9: para cada vault, los
//! descendientes de cada tag, los avisos y lo que muestra el panel.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::{key, tags};
use crate::tags::{Scope, ScopeFilter, TagKey, TagProblem, TagTree, Tags};

fn names<'k>(keys: impl IntoIterator<Item = &'k TagKey>) -> String {
    keys.into_iter()
        .map(TagKey::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// El panel como texto: raíces y, bajo cada tag, sus hijos y sus notas. Un
/// tag que ya está en el camino se marca y no se expande otra vez (05 §7).
pub(super) fn panel(tags: &Tags, scope: &ScopeFilter<'_>) -> String {
    let tree = tags.tree(scope);
    let mut out = String::new();
    let mut walk = Walk {
        tags,
        scope,
        tree: &tree,
        path: Vec::new(),
        out: &mut out,
    };
    for root in tree.roots() {
        walk.branch(root, 0);
    }
    out
}

struct Walk<'a> {
    tags: &'a Tags,
    scope: &'a ScopeFilter<'a>,
    tree: &'a TagTree,
    path: Vec<TagKey>,
    out: &'a mut String,
}

impl Walk<'_> {
    fn branch(&mut self, key: &TagKey, depth: usize) {
        let node = self.tree.node(key).unwrap();
        let cycle = self.path.contains(key);
        let mark = if cycle {
            " ↻"
        } else if node.only_parent {
            " (solo como padre)"
        } else {
            ""
        };
        let indent = "  ".repeat(depth);
        writeln!(self.out, "{indent}{} ({}){mark}", node.name, node.count).unwrap();
        if cycle {
            return;
        }
        self.path.push(key.clone());
        for child in &node.children {
            self.branch(child, depth + 1);
        }
        for note in self.tags.tagged_notes(key, self.scope) {
            writeln!(self.out, "{indent}  · {note}").unwrap();
        }
        self.path.pop();
    }
}

fn problem(problem: &TagProblem) -> String {
    match problem {
        TagProblem::InvalidPageName { path } => format!("nombre inválido: {path}"),
        TagProblem::DuplicatePage { kept, ignored } => {
            format!("página repetida: vale {kept}, no {ignored}")
        }
        TagProblem::InvalidValue { page, value } => {
            format!("{page}: {:?} no se entiende: {:?}", value.field, value.text)
        }
        TagProblem::AliasIsPage { page, alias, other } => {
            format!("{page}: el alias {alias} es la página {other}")
        }
        TagProblem::AliasConflict { alias, pages } => {
            let pages: Vec<String> = pages.iter().map(ToString::to_string).collect();
            format!("alias {alias} en varias páginas: {}", pages.join(", "))
        }
        TagProblem::SelfParent { page } => format!("{page}: es su propio padre"),
        TagProblem::Cycle { path } => format!(
            "ciclo: {}",
            path.iter()
                .map(TagKey::as_str)
                .collect::<Vec<_>>()
                .join(" → ")
        ),
    }
}

fn render(tags: &Tags) -> String {
    let mut out = String::from("panel:\n");
    out.push_str(&panel(tags, &Scope::default().filter(tags)));
    let nodes: BTreeSet<&TagKey> = tags
        .index()
        .keys()
        .chain(tags.graph().pages().map(|page| &page.key))
        .chain(tags.graph().parent_keys())
        .collect();
    out.push_str("tags:\n");
    for node in nodes {
        let graph = tags.graph();
        let resolved = graph.resolve(node);
        if resolved == node {
            writeln!(
                out,
                "  {node}: padres [{}] alias [{}] trae [{}]",
                names(graph.parents(node)),
                names(graph.aliases(node)),
                names(&graph.expansion(node)),
            )
            .unwrap();
        } else {
            writeln!(out, "  {node}: alias de {resolved}").unwrap();
        }
    }
    out.push_str("avisos:\n");
    for item in tags.graph().problems() {
        writeln!(out, "  {}", problem(item)).unwrap();
    }
    out
}

type Case = (&'static str, &'static [(&'static str, &'static str)]);

const CASES: &[Case] = &[
    (
        "several_parents",
        &[
            ("tags/gpui.md", "---\nparents: [rust, graficos, ui]\n---\n"),
            ("tags/rust.md", "---\nparents: lenguajes\n---\n"),
            ("a.md", "#gpui"),
            ("b.md", "#rust #ui"),
            ("c.md", "#lenguajes"),
        ],
    ),
    (
        "alias_is_a_page",
        &[
            ("tags/rust.md", "---\naliases: [rs, Rust]\n---\n"),
            ("tags/rs.md", ""),
            ("a.md", "#rs"),
            ("b.md", "#rust"),
        ],
    ),
    (
        "alias_used_without_page",
        &[
            ("tags/rust.md", "---\naliases: [\"#rs\"]\n---\n"),
            ("a.md", "#rs"),
            ("b.md", "#RS #Rust"),
        ],
    ),
    (
        "alias_conflict",
        &[
            ("tags/rust.md", "---\naliases: [rs]\n---\n"),
            ("tags/rescript.md", "---\naliases: rs\n---\n"),
            ("a.md", "#rs"),
        ],
    ),
    (
        "parent_only",
        &[
            ("tags/gpui.md", "---\nparents: [graficos, rsut]\n---\n"),
            ("a.md", "#gpui"),
        ],
    ),
    (
        "parent_through_alias",
        &[
            ("tags/rust.md", "---\naliases: [rs]\n---\n"),
            ("tags/gpui.md", "---\nparents: [rs]\n---\n"),
            ("a.md", "#gpui"),
        ],
    ),
    (
        "cycle",
        &[
            ("tags/a.md", "---\nparents: [b]\n---\n"),
            ("tags/b.md", "---\nparents: [c]\n---\n"),
            ("tags/c.md", "---\nparents: [a]\n---\n"),
            ("x.md", "#a"),
            ("y.md", "#c"),
        ],
    ),
    (
        "cycle_with_outside_parent",
        &[
            ("tags/a.md", "---\nparents: [b, raiz]\n---\n"),
            ("tags/b.md", "---\nparents: [a]\n---\n"),
            ("x.md", "#b"),
        ],
    ),
    (
        "self_parent",
        &[("tags/a.md", "---\nparents: [a, b]\n---\n"), ("x.md", "#a")],
    ),
    (
        "page_files",
        &[
            ("tags/Rust.md", ""),
            ("tags/rust.md", "---\ncolor: rojo\n---\n"),
            ("tags/mi tag.md", "#rust"),
            ("tags/viejo/x.md", "#rust"),
            (
                "tags/raro.md",
                "---\ncolor: \"rojo oscuro\"\nicon: libro\naliases: [a/b]\n---\n",
            ),
            ("a.md", "#rust"),
        ],
    ),
    (
        "spellings",
        &[
            ("tags/Rust.md", ""),
            ("a.md", "#rust #rust"),
            ("b.md", "#RUST"),
            ("c.md", "#Ñandú #ñandú"),
        ],
    ),
];

#[test]
fn graph_cases() {
    for (name, files) in CASES {
        insta::assert_snapshot!(*name, render(&tags(files)));
    }
}

#[test]
fn a_page_without_notes_shows_its_file_name() {
    let tags = tags(&[("tags/GPUI.md", "")]);
    assert_eq!(tags.display_name(&key("gpui")), Some("GPUI"));
    assert_eq!(tags.count(&key("gpui")), 0);
}

#[test]
fn the_page_name_counts_as_one_more() {
    let tags = tags(&[("tags/Rust.md", ""), ("a.md", "#rust"), ("b.md", "#Rust")]);
    assert_eq!(tags.display_name(&key("rust")), Some("Rust"));
}

#[test]
fn a_tie_goes_to_the_first_in_byte_order() {
    let tags = tags(&[("a.md", "#rust"), ("b.md", "#Rust")]);
    assert_eq!(tags.display_name(&key("rust")), Some("Rust"));
}

#[test]
fn page_bodies_do_not_count() {
    let tags = tags(&[("tags/rust.md", "Ver #gpui"), ("a.md", "#rust")]);
    assert_eq!(tags.count(&key("gpui")), 0);
    assert!(tags.index().tag(&key("gpui")).is_none());
    assert_eq!(tags.count(&key("rust")), 1);
}
