//! Un vault en memoria para los tests del índice de links: ruta → contenido.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use nota_vault::{EntryKind, LinkIndex, LinkTarget, VaultPath};
use proptest::prelude::*;

pub type Model = BTreeMap<VaultPath, String>;

pub fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

pub fn kind(path: &VaultPath) -> EntryKind {
    let is_note = path
        .name()
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("md"));
    if is_note {
        EntryKind::Note
    } else {
        EntryKind::Attachment
    }
}

pub fn model(files: &[(&str, &str)]) -> Model {
    files
        .iter()
        .map(|(file, text)| (path(file), (*text).to_owned()))
        .collect()
}

pub fn build(model: &Model) -> LinkIndex {
    LinkIndex::build(
        model.keys().map(|file| (file.clone(), kind(file))),
        |file| model.get(file).cloned(),
    )
}

pub fn describe(target: &LinkTarget) -> String {
    match target {
        LinkTarget::SameNote => "la misma nota".to_owned(),
        LinkTarget::Resolved(path) => path.to_string(),
        LinkTarget::Ambiguous(paths) => {
            let paths: Vec<&str> = paths.iter().map(VaultPath::as_str).collect();
            format!("ambiguo: {}", paths.join(", "))
        }
        LinkTarget::Unresolved => "sin resolver".to_owned(),
    }
}

/// Cada nota con sus links salientes y sus backlinks, legible en un snapshot.
pub fn report(model: &Model, index: &LinkIndex) -> String {
    let mut out = String::new();
    for (file, text) in model {
        writeln!(out, "{file}").unwrap();
        for link in index.outgoing(file) {
            let written = &text[link.span.clone()];
            writeln!(out, "  → {written} → {}", describe(&link.target)).unwrap();
        }
        for back in index.backlinks(file) {
            let context = &back.link.context.text;
            writeln!(
                out,
                "  ← {} línea {}: {context}",
                back.source,
                back.link.line + 1
            )
            .unwrap();
        }
    }
    out
}

pub fn file_path() -> impl Strategy<Value = String> {
    let folder = prop::collection::vec(prop::sample::select(vec!["a", "b"]), 0..=2);
    let name = prop::sample::select(vec!["x.md", "X.md", "y.md", "m n.md", "z.png", "w"]);
    (folder, name).prop_map(|(mut parts, name)| {
        parts.push(name);
        parts.join("/")
    })
}

/// Un archivo o una carpeta.
pub fn any_path() -> impl Strategy<Value = String> {
    prop_oneof![
        file_path(),
        prop::sample::select(vec![
            "a".to_owned(),
            "b".to_owned(),
            "a/b".to_owned(),
            "b/a".to_owned()
        ]),
    ]
}

pub fn content() -> impl Strategy<Value = String> {
    let piece = prop::sample::select(vec![
        "[[x]]",
        "[[X]]",
        "[[x.md]]",
        "[[a/x]]",
        "[[/a/x]]",
        "[[./x]]",
        "[[../x]]",
        "[[y#H]]",
        "[[y|alias]]",
        "![[z.png]]",
        "[[w]]",
        "[[m n]]",
        "[t](x.md)",
        "[t](<a/y.md>)",
        "[t](../x.md#H)",
        "[t](m%20n.md)",
        "`[[x]]`",
        "\n```\n[[y]]\n```\n",
        " texto ",
        "\n",
        "[[#H]]",
        "[[falta]]",
    ]);
    prop::collection::vec(piece, 0..8).prop_map(|pieces| pieces.concat())
}
