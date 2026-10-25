#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use metalmind_vault::{EntryKind, LinkTarget, RenamePlan, RenamePlanError, VaultPath};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::rstest;
use support::links::{Model, any_path, build, content, file_path, kind, model, path};

/// Aplica el plan y el rename al modelo; devuelve el modelo nuevo y lo que
/// deshace las ediciones.
fn execute(model: &Model, plan: &RenamePlan) -> (Model, Model) {
    let mut edited = model.clone();
    for file in &plan.files {
        let text = edited.get_mut(&file.path).unwrap();
        let (new, undo) = file.apply(text).unwrap();
        assert_eq!(undo.apply(&new).unwrap().0, *text, "deshacer {}", file.path);
        *text = new;
    }
    let renamed = edited
        .iter()
        .map(|(file, text)| {
            let file = file
                .rebase(&plan.from, &plan.to)
                .unwrap_or_else(|| file.clone());
            (file, text.clone())
        })
        .collect();
    (renamed, edited)
}

/// El texto de `note` tras renombrar `from` a `to`.
fn renamed(files: &[(&str, &str)], from: &str, to: &str, note: &str) -> String {
    let model = model(files);
    let plan = build(&model).rename_plan(&path(from), &path(to)).unwrap();
    assert_eq!(plan.conflicts, []);
    let (after, _) = execute(&model, &plan);
    let note = path(note)
        .rebase(&path(from), &path(to))
        .unwrap_or_else(|| path(note));
    after[&note].clone()
}

#[rstest]
#[case::nombre(
    "[[nota]] [[nota|alias]] [[nota#H]]",
    "nueva.md",
    "[[nueva]] [[nueva|alias]] [[nueva#H]]"
)]
#[case::con_extension("[[nota.md]] ![[nota.md]]", "nueva.md", "[[nueva.md]] ![[nueva.md]]")]
#[case::mover_no_cambia_un_nombre_unico("[[nota]]", "c/nota.md", "[[nota]]")]
#[case::ruta_parcial("[[a/nota]]", "b/nueva.md", "[[b/nueva]]")]
#[case::desde_la_raiz("[[/a/nota]]", "b/nueva.md", "[[/b/nueva]]")]
#[case::markdown(
    "[x](a/nota.md) [y](<a/nota.md#H>)",
    "b/Mi Nota.md",
    "[x](b/Mi%20Nota.md) [y](<b/Mi Nota.md#H>)"
)]
#[case::markdown_sin_ruta("[x](nota.md)", "Mi Nota.md", "[x](Mi%20Nota.md)")]
#[case::salto_de_linea_en_el_nombre(
    "[x](<a/nota.md>) [y](a/nota.md)",
    "dos\nlíneas.md",
    "[x](<dos%0Alíneas.md>) [y](dos%0Alíneas.md)"
)]
#[case::en_codigo_no(
    "`[[nota]]`\n```\n[[nota]]\n```\n[[nota]]",
    "nueva.md",
    "`[[nota]]`\n```\n[[nota]]\n```\n[[nueva]]"
)]
#[case::otra_nota_no("[[otra]] [[nota]]", "nueva.md", "[[otra]] [[nueva]]")]
fn links_keep_their_form(#[case] text: &str, #[case] to: &str, #[case] expected: &str) {
    let files = [("inicio.md", text), ("a/nota.md", ""), ("otra.md", "")];
    assert_eq!(renamed(&files, "a/nota.md", to, "inicio.md"), expected);
}

#[test]
fn relative_links_follow_the_moved_note() {
    let files = [
        ("a/nota.md", "[[./hermana]] [x](../raiz.md) [[hermana]]"),
        ("a/hermana.md", "[[./nota]]"),
        ("raiz.md", ""),
    ];
    assert_eq!(
        renamed(&files, "a/nota.md", "b/c/nota.md", "a/nota.md"),
        "[[../../a/hermana]] [x](../../raiz.md) [[hermana]]"
    );
    assert_eq!(
        renamed(&files, "a/nota.md", "b/c/nota.md", "a/hermana.md"),
        "[[../b/c/nota]]"
    );
    assert_eq!(renamed(&files, "a", "z", "a/hermana.md"), "[[./nota]]");
}

#[test]
fn a_new_name_that_would_capture_other_links_lengthens_them() {
    let files = [
        ("inicio.md", "[[plan]]"),
        ("viejo/plan.md", ""),
        ("x.md", ""),
    ];
    assert_eq!(
        renamed(&files, "x.md", "plan.md", "inicio.md"),
        "[[viejo/plan]]"
    );
}

#[test]
fn a_name_that_now_ties_gets_more_of_its_path() {
    let files = [("inicio.md", "[[nota]]"), ("a/nota.md", ""), ("b/x.md", "")];
    assert_eq!(
        renamed(&files, "b/x.md", "b/nota.md", "inicio.md"),
        "[[a/nota]]"
    );
}

#[test]
fn a_wikilink_cannot_name_a_file_with_a_hash() {
    let model = model(&[("inicio.md", "[[nota]] [x](nota.md)"), ("nota.md", "")]);
    let plan = build(&model)
        .rename_plan(&path("nota.md"), &path("nota #1.md"))
        .unwrap();
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].span, 0..8);
    assert_eq!(plan.files[0].edits[0].new, "nota%20\\#1.md");
}

#[test]
fn the_destination_must_be_free() {
    let index = build(&model(&[("a/x.md", ""), ("b.md", "")]));
    assert_eq!(
        index.rename_plan(&path("a/x.md"), &path("b.md")),
        Err(RenamePlanError::Exists(path("b.md")))
    );
    assert!(matches!(
        index.rename_plan(&path("a"), &path("a/c")),
        Err(RenamePlanError::IntoItself { .. })
    ));
    assert_eq!(
        index.rename_plan(&path("a/x.md"), &path("b.md/c/x.md")),
        Err(RenamePlanError::NotAFolder(path("b.md")))
    );
}

/// Dónde resuelve cada link de cada nota, por nota y posición.
fn targets(model: &Model) -> Vec<(VaultPath, usize, LinkTarget)> {
    let index = build(model);
    model
        .keys()
        .flat_map(|file| {
            index
                .outgoing(file)
                .iter()
                .enumerate()
                .map(|(ordinal, link)| (file.clone(), ordinal, link.target.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Tras aplicar el plan y renombrar `from` a `to`, cada link que resolvía a
/// un archivo resuelve a ese mismo archivo en su ruta nueva, salvo los
/// conflictos. Un rename que no se puede planear no tiene nada que comprobar.
fn keeps_every_resolved_target(
    model: &Model,
    from: &VaultPath,
    to: &VaultPath,
) -> Result<(), TestCaseError> {
    let index = build(model);
    let Ok(plan) = index.rename_plan(from, to) else {
        return Ok(());
    };
    if to.ancestors().any(|folder| model.contains_key(&folder)) {
        return Ok(());
    }
    let (after, _) = execute(model, &plan);
    let moved = |file: &VaultPath| file.rebase(from, to).unwrap_or_else(|| file.clone());
    let now: std::collections::BTreeMap<_, _> = targets(&after)
        .into_iter()
        .map(|(file, ordinal, target)| ((file, ordinal), target))
        .collect();
    for (file, ordinal, target) in targets(model) {
        let LinkTarget::Resolved(target) = target else {
            continue;
        };
        let conflict = plan
            .conflicts
            .iter()
            .any(|c| c.source == file && index.outgoing(&file)[ordinal].span == c.span);
        // Una nota que pasa a ser adjunto ya no tiene links.
        if conflict || kind(&moved(&file)) != EntryKind::Note {
            continue;
        }
        prop_assert_eq!(
            now.get(&(moved(&file), ordinal)),
            Some(&LinkTarget::Resolved(moved(&target))),
            "link {} de {} con {:?}",
            ordinal,
            file,
            plan
        );
    }
    Ok(())
}

fn vault() -> impl Strategy<Value = Model> {
    prop::collection::btree_map(file_path(), content(), 1..10).prop_map(|files| {
        let mut model = Model::new();
        for (file, text) in files {
            let file = path(&file);
            let clash = model
                .keys()
                .any(|other| other.starts_with(&file) || file.starts_with(other));
            if !clash {
                model.insert(file, text);
            }
        }
        model
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Ver [`keeps_every_resolved_target`].
    #[test]
    fn every_resolved_link_keeps_its_target(
        model in vault(),
        file in any::<prop::sample::Index>(),
        folder in prop::option::weighted(0.3, any_path()),
        to in any_path(),
    ) {
        let files: Vec<&VaultPath> = model.keys().collect();
        let from = folder.map_or_else(|| file.get(&files).to_owned().clone(), |folder| path(&folder));
        keeps_every_resolved_target(&model, &from, &path(&to))?;
    }
}
