#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

mod support;

use nota_vault::{
    Destination, EntryKind, LinkIndex, LinkSubpath, LinkTarget, VaultChange, VaultPath,
};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use support::links::{
    Model, any_path, build, content, describe, file_path, kind, model, path, report,
};

const FIXTURE: &[(&str, &str)] = &[
    (
        "Inicio.md",
        "# Inicio\n\nVer [[nota]], [[proyectos/Plan#Fechas]] y [[Plan#^tarea]].\n\
         ![[diagrama.png|300]] [texto](proyectos/nota.md) [web](https://example.com)\n\
         `[[en código]]` y [[No existe]] y [[duplicada]] y [[#Inicio]]\n",
    ),
    ("nota.md", "Vuelvo a [[Inicio]] y a [[./proyectos/nota]].\n"),
    (
        "proyectos/nota.md",
        "[[../nota]] [[duplicada]]\n```\n[[Inicio]]\n```\n",
    ),
    ("proyectos/Plan.md", "[a](<../Inicio.md>) [[Plan]]\n"),
    ("a/duplicada.md", ""),
    ("b/duplicada.md", ""),
    ("proyectos/duplicada.md", ""),
    ("adjuntos/diagrama.png", ""),
];

#[test]
fn fixture() {
    let model = model(FIXTURE);
    insta::assert_snapshot!(report(&model, &build(&model)));
}

#[test]
fn unresolved_and_ambiguous_are_listed_by_note() {
    let model = model(FIXTURE);
    let index = build(&model);
    let unresolved: Vec<String> = index
        .unresolved()
        .iter()
        .map(|found| format!("{}: {}", found.source, found.link.target_text))
        .collect();
    assert_eq!(unresolved, ["Inicio.md: No existe"]);
    let ambiguous: Vec<String> = index
        .ambiguous()
        .iter()
        .map(|found| format!("{}: {}", found.source, describe(&found.link.target)))
        .collect();
    assert_eq!(
        ambiguous,
        ["Inicio.md: ambiguo: a/duplicada.md, b/duplicada.md, proyectos/duplicada.md"]
    );
}

#[test]
fn follow_the_link_under_the_cursor() {
    let index = build(&model(FIXTURE));
    let source = path("Inicio.md");
    let text = "[[proyectos/Plan#Fechas]] y [x](<Mi Nota.md#^b>) y [[#Arriba]]";
    assert_eq!(
        index.follow(&source, text, 3),
        Some(Destination {
            target: LinkTarget::Resolved(path("proyectos/Plan.md")),
            target_text: "proyectos/Plan".to_owned(),
            subpath: Some(LinkSubpath::Heading("Fechas".to_owned())),
        })
    );
    assert_eq!(
        index.follow(&source, text, 30),
        Some(Destination {
            target: LinkTarget::Unresolved,
            target_text: "Mi Nota.md".to_owned(),
            subpath: Some(LinkSubpath::Block("b".to_owned())),
        })
    );
    let same = index.follow(&source, text, text.len() - 1).unwrap();
    assert_eq!(same.target, LinkTarget::SameNote);
    assert_eq!(index.follow(&source, text, 26), None);
}

/// Una operación sobre el vault en memoria.
#[derive(Debug, Clone)]
enum Op {
    Write(String, String),
    Remove(String),
    Rename(String, String),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (file_path(), content()).prop_map(|(path, text)| Op::Write(path, text)),
        1 => any_path().prop_map(Op::Remove),
        2 => (any_path(), any_path()).prop_map(|(from, to)| Op::Rename(from, to)),
    ]
}

fn under(model: &Model, prefix: &VaultPath) -> Vec<VaultPath> {
    model
        .keys()
        .filter(|file| file.starts_with(prefix))
        .cloned()
        .collect()
}

/// Un archivo no puede estar donde va una carpeta ni al revés.
fn fits(model: &Model, file: &VaultPath) -> bool {
    !file.ancestors().any(|folder| model.contains_key(&folder))
        && under(model, file).iter().all(|found| found == file)
}

/// Aplica `op` al modelo y devuelve los cambios que vería el watcher, o
/// nada si no se puede hacer.
fn apply(model: &mut Model, op: &Op) -> Vec<VaultChange> {
    match op {
        Op::Write(file, text) => {
            let file = path(file);
            if !fits(model, &file) {
                return Vec::new();
            }
            let change = if model.contains_key(&file) {
                VaultChange::Modified { path: file.clone() }
            } else {
                VaultChange::Created {
                    path: file.clone(),
                    kind: kind(&file),
                }
            };
            model.insert(file, text.clone());
            vec![change]
        }
        Op::Remove(target) => {
            let target = path(target);
            under(model, &target)
                .into_iter()
                .map(|file| {
                    model.remove(&file);
                    VaultChange::Removed {
                        kind: kind(&file),
                        path: file,
                    }
                })
                .collect()
        }
        Op::Rename(from, to) => {
            let (from, to) = (path(from), path(to));
            let moved = under(model, &from);
            if moved.is_empty() || to.starts_with(&from) || !under(model, &to).is_empty() {
                return Vec::new();
            }
            let mut rest = model.clone();
            let files: Vec<_> = moved
                .iter()
                .map(|old| (old.rebase(&from, &to).unwrap(), rest.remove(old).unwrap()))
                .collect();
            if files.iter().any(|(new, _)| !fits(&rest, new)) {
                return Vec::new();
            }
            let kind = if model.contains_key(&from) {
                kind(&from)
            } else {
                EntryKind::Folder
            };
            rest.extend(files);
            *model = rest;
            vec![VaultChange::Renamed { from, to, kind }]
        }
    }
}

proptest! {
    /// Aplicar los cambios del watcher da el mismo índice que armarlo de cero.
    #[test]
    fn incremental_equals_rebuild(ops in prop::collection::vec(op(), 1..24)) {
        let mut model = Model::new();
        let mut index = LinkIndex::default();
        for op in &ops {
            let changes = apply(&mut model, op);
            for change in &changes {
                index.apply(change, |file| model.get(file).cloned());
            }
            prop_assert_eq!(&index, &build(&model), "tras {:?}", op);
        }
    }
}
