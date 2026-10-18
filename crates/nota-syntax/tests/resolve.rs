#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fmt::Write as _;

use nota_syntax::{Resolution, resolve, resolve_from};

const VAULT: &[&str] = &[
    "Inicio.md",
    "nota.md",
    "proyectos/nota.md",
    "proyectos/Plan.md",
    "archivo/2023/Plan.md",
    "diario/2024-10-01.md",
    "a/Duplicada.md",
    "b/Duplicada.md",
    "c/d/Duplicada.md",
    "adjuntos/diagrama.png",
    "adjuntos/notas.md.md",
    "Mayúsculas/Árbol.md",
    "Casa.md",
    "casa.md",
];

const TARGETS: &[&str] = &[
    "nota",
    "nota.md",
    "NOTA",
    "proyectos/nota",
    "Proyectos/Nota.md",
    "/nota",
    "/proyectos/nota",
    "/Plan",
    "plan",
    "2023/plan",
    "chivo/2023/plan",
    "2024-10-01",
    "duplicada",
    "a/duplicada",
    "d/duplicada",
    "diagrama.png",
    "diagrama",
    "notas.md",
    "árbol",
    "mayúsculas/ÁRBOL",
    "Casa",
    "casa.md",
    "CASA",
    "Duplicada",
    "  nota  ",
    "no existe",
    "",
    "/",
];

fn describe(resolution: Resolution) -> String {
    match resolution {
        Resolution::Resolved(path) => path.to_owned(),
        Resolution::Ambiguous(paths) => format!("ambiguo: {}", paths.join(", ")),
        Resolution::Unresolved => "sin resolver".to_owned(),
    }
}

/// La tabla de resolución es la spec: cada destino contra el mismo vault.
#[test]
fn resolution_table() {
    let mut table = String::new();
    for target in TARGETS {
        let resolution = describe(resolve(target, VAULT.iter().copied()));
        writeln!(table, "{target:?} → {resolution}").unwrap();
    }
    insta::assert_snapshot!(table);
}

/// Destinos escritos en `proyectos/Plan.md`: los relativos parten de su
/// carpeta y el resto se resuelve igual que sin origen.
const RELATIVE_TARGETS: &[&str] = &[
    "./nota",
    "./nota.md",
    "./Plan",
    "../nota.md",
    "../Inicio",
    "../proyectos/./nota",
    "../archivo/2023/Plan.md",
    "./../diario/2024-10-01",
    "./no-existe",
    "../../nota",
    "nota",
    "duplicada",
];

#[test]
fn relative_resolution_table() {
    let mut table = String::new();
    for target in RELATIVE_TARGETS {
        let resolution = describe(resolve_from(
            "proyectos/Plan.md",
            target,
            VAULT.iter().copied(),
        ));
        writeln!(table, "{target:?} → {resolution}").unwrap();
    }
    insta::assert_snapshot!(table);
}

mod keys {
    use nota_syntax::{path_keys, resolve_from, target_key};
    use proptest::prelude::*;

    fn path() -> impl Strategy<Value = String> {
        let part =
            prop::sample::select(vec!["a", "A", "b", "x.md", "X.MD", "x.md.md", "y.png", "é"]);
        prop::collection::vec(part, 1..=3).prop_map(|parts| parts.join("/"))
    }

    fn target() -> impl Strategy<Value = String> {
        let part = prop::sample::select(vec![
            "a", "A", "b", "x", "X", "x.md", "x.MD", "y.png", "é", ".", "..", "",
        ]);
        (
            prop::sample::select(vec!["", "/", "./", "../", " "]),
            prop::collection::vec(part, 1..=3),
        )
            .prop_map(|(prefix, parts)| format!("{prefix}{}", parts.join("/")))
    }

    proptest! {
        /// Un índice que solo le pasa a `resolve_from` las rutas con la
        /// clave del destino resuelve igual que con todo el vault.
        #[test]
        fn the_key_never_hides_a_match(
            paths in prop::collection::btree_set(path(), 0..12),
            source in path(),
            target in target(),
        ) {
            let all: Vec<&str> = paths.iter().map(String::as_str).collect();
            let key = target_key(&source, &target);
            let candidates: Vec<&str> = all
                .iter()
                .copied()
                .filter(|path| key.as_ref().is_some_and(|key| path_keys(path).any(|k| &k == key)))
                .collect();
            prop_assert_eq!(
                resolve_from(&source, &target, all.iter().copied()),
                resolve_from(&source, &target, candidates)
            );
        }
    }
}
