//! La plantilla que piden los tags de una nota nueva: el `template:` de la
//! página de cada tag, con los alias ya resueltos por el `TagGraph` (05 §7).

use metalmind_vault::tags::{TagGraph, TagKey};

/// Qué plantilla piden los tags de una nota nueva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagChoice<'a> {
    None,
    Template {
        template: &'a str,
        /// El nombre de la página que la pide.
        tag: &'a str,
    },
    /// Tags que piden plantillas distintas, como `(página, plantilla)`.
    /// Elegir una adivinaría, así que no se usa ninguna.
    Conflict(Vec<(&'a str, &'a str)>),
}

/// La plantilla que piden `tags` entre todos. Un alias pide la de su tag, y
/// dos tags que piden la misma no son un conflicto. Un tag no hereda la
/// plantilla de sus `parents`.
#[must_use]
pub fn tag_template<'g>(graph: &'g TagGraph, tags: &[String]) -> TagChoice<'g> {
    let mut asked: Vec<(&TagKey, &str, &str)> = Vec::new();
    for tag in tags {
        let Some(key) = TagKey::parse(tag) else {
            continue;
        };
        let Some(page) = graph.page(graph.resolve(&key)) else {
            continue;
        };
        let Some(template) = page.template.as_deref() else {
            continue;
        };
        if !asked.iter().any(|&(seen, ..)| *seen == page.key) {
            asked.push((&page.key, &page.name, template));
        }
    }
    match asked.as_slice() {
        [] => TagChoice::None,
        [(_, tag, template), rest @ ..] if rest.iter().all(|(.., other)| other == template) => {
            TagChoice::Template { template, tag }
        }
        _ => TagChoice::Conflict(
            asked
                .into_iter()
                .map(|(_, tag, template)| (tag, template))
                .collect(),
        ),
    }
}

/// Los tags cuya página pide `template`, por su nombre de página: los que
/// se le ponen a una nota creada desde ella.
#[must_use]
pub fn tags_asking_for<'g>(graph: &'g TagGraph, template: &str) -> Vec<&'g str> {
    graph
        .pages()
        .filter(|page| page.template.as_deref() == Some(template))
        .map(|page| page.name.as_str())
        .collect()
}

/// El tag que se le pone a una nota creada desde `template`: el de la
/// única página que la pide. Si la piden varias, ninguno; elegir adivinaría.
#[must_use]
pub fn tag_of_template<'g>(graph: &'g TagGraph, template: &str) -> Option<&'g str> {
    match tags_asking_for(graph, template).as_slice() {
        [tag] => Some(tag),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::test_vault::graph;

    fn pages() -> TagGraph {
        graph(&[
            ("tags/reunion.md", "---\ntemplate: reunión\n---\n"),
            ("tags/trabajo.md", "---\ntemplate: reunión\n---\n"),
            (
                "tags/Diario.md",
                "---\ntemplate: diario\naliases: [bitacora]\n---\n",
            ),
            ("tags/rust.md", "Sin plantilla.\n"),
            ("tags/straße.md", "---\ntemplate: calle\n---\n"),
            ("tags/cancio\u{301}n.md", "---\ntemplate: música\n---\n"),
            (
                "tags/semanal.md",
                "---\nparents: [reunion]\n---\nHereda nada.\n",
            ),
        ])
    }

    fn tags(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    #[rstest]
    #[case::no_tags(&[], TagChoice::None)]
    #[case::without_page(&["nuevo"], TagChoice::None)]
    #[case::page_without_template(&["rust"], TagChoice::None)]
    #[case::invalid_tag(&["con espacio"], TagChoice::None)]
    #[case::one(&["rust", "reunion"], TagChoice::Template { template: "reunión", tag: "reunion" })]
    #[case::case_insensitive(&["DIARIO"], TagChoice::Template { template: "diario", tag: "Diario" })]
    #[case::hash(&["#diario"], TagChoice::Template { template: "diario", tag: "Diario" })]
    #[case::alias(&["bitacora"], TagChoice::Template { template: "diario", tag: "Diario" })]
    #[case::full_folding(&["STRASSE"], TagChoice::Template { template: "calle", tag: "straße" })]
    #[case::decomposed_page_name(&["canci\u{F3}n"], TagChoice::Template { template: "música", tag: "cancio\u{301}n" })]
    #[case::no_inheritance_from_parents(&["semanal"], TagChoice::None)]
    #[case::same_template(&["reunion", "trabajo"], TagChoice::Template { template: "reunión", tag: "reunion" })]
    #[case::alias_and_its_tag(&["diario", "bitacora"], TagChoice::Template { template: "diario", tag: "Diario" })]
    #[case::conflict(
        &["diario", "reunion", "Diario"],
        TagChoice::Conflict(vec![("Diario", "diario"), ("reunion", "reunión")])
    )]
    fn choice(#[case] names: &[&str], #[case] expected: TagChoice) {
        let pages = pages();
        assert_eq!(tag_template(&pages, &tags(names)), expected);
    }

    #[rstest]
    #[case::two_tags_ask_for_it("reunión", &["reunion", "trabajo"])]
    #[case::one_with_aliases("diario", &["Diario"])]
    #[case::none("semanal", &[])]
    #[case::exact_name("Diario", &[])]
    fn asking_for(#[case] template: &str, #[case] expected: &[&str]) {
        let pages = pages();
        let mut asking = tags_asking_for(&pages, template);
        asking.sort_unstable();
        assert_eq!(asking, expected);
    }

    #[rstest]
    #[case::one("diario", Some("Diario"))]
    #[case::two_is_a_guess("reunión", None)]
    #[case::none("semanal", None)]
    fn tag_of(#[case] template: &str, #[case] expected: Option<&str>) {
        assert_eq!(tag_of_template(&pages(), template), expected);
    }

    #[test]
    fn an_invalid_template_is_a_page_problem_and_asks_for_nothing() {
        let pages = graph(&[("tags/a.md", "---\ntemplate: ../fuera\n---\n")]);
        assert_eq!(tag_template(&pages, &tags(&["a"])), TagChoice::None);
        assert_eq!(pages.problems().len(), 1, "{:?}", pages.problems());
    }
}
