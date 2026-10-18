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
