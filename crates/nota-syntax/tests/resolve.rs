#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fmt::Write as _;

use nota_syntax::{Resolution, resolve};

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
    "  nota  ",
    "no existe",
    "",
    "/",
];

/// La tabla de resolución es la spec: cada destino contra el mismo vault.
#[test]
fn resolution_table() {
    let mut table = String::new();
    for target in TARGETS {
        let resolution = match resolve(target, VAULT.iter().copied()) {
            Resolution::Resolved(path) => path.to_owned(),
            Resolution::Ambiguous(paths) => format!("ambiguo: {}", paths.join(", ")),
            Resolution::Unresolved => "sin resolver".to_owned(),
        };
        writeln!(table, "{target:?} → {resolution}").unwrap();
    }
    insta::assert_snapshot!(table);
}
