use std::collections::HashMap;

use metalmind_vault::EntryKind;
use pretty_assertions::assert_eq;

use super::*;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Las notas con su texto; lo que no termina en `.md` es un adjunto.
fn index(notes: &[(&str, &str)]) -> Arc<LinkIndex> {
    let texts: HashMap<VaultPath, String> = notes
        .iter()
        .map(|&(name, text)| (path(name), text.to_owned()))
        .collect();
    Arc::new(LinkIndex::build(
        texts
            .keys()
            .map(|name| (name.clone(), EntryKind::of_file(name))),
        |name| texts.get(name).cloned(),
    ))
}

/// `diario.md` enlaza dos veces a `plan.md`, una a `ideas/b.md`, a un
/// heading propio, a una nota que no existe y a un nombre que empata.
fn vault() -> Arc<LinkIndex> {
    index(&[
        ("plan.md", "# Plan\n"),
        ("ideas/b.md", "# B\n"),
        ("a/idea.md", "uno\n"),
        ("c/idea.md", "dos\n"),
        (
            "diario.md",
            "# Hoy\n[[falta]] y el [[plan]]\nver [[b]] y [[#Hoy]]\n[[idea]] y el [[plan#Meta]]\n",
        ),
    ])
}

fn outline(note: &str) -> OutgoingOutline {
    let mut outline = OutgoingOutline::new(vault());
    outline.show_note(Some(path(note)));
    outline.refresh();
    outline
}

fn shown(outline: &OutgoingOutline) -> Vec<String> {
    outline
        .rows()
        .iter()
        .map(|row| {
            let name = match &row.destination {
                Destination::File(path) => path.as_str().to_owned(),
                Destination::Ambiguous(text) => format!("{text}?"),
                Destination::Missing(text) => format!("{text}!"),
            };
            format!(
                "{name} ({}) {}:{}",
                row.count,
                row.first.line + 1,
                row.first.column
            )
        })
        .collect()
}

/// Un destino por fila, en el orden de su primer link, y al final lo que no
/// lleva a una sola nota; el heading de la misma nota no sale.
#[test]
fn un_destino_por_fila_y_al_final_lo_que_no_lleva_a_una_nota() {
    assert_eq!(
        shown(&outline("diario.md")),
        [
            "plan.md (2) 2:15",
            "ideas/b.md (1) 3:4",
            "falta! (1) 2:0",
            "idea? (1) 4:0",
        ]
    );
    assert_eq!(outline("plan.md").rows(), []);
}

/// Una nota abre esa nota donde se dejó; lo que no lleva a una abre la
/// activa en su link.
#[test]
fn cada_fila_abre_su_destino_o_el_link_que_hay_que_arreglar() {
    let mut outline = outline("diario.md");
    assert_eq!(outline.selected_file(), Some(&path("plan.md")));
    assert_eq!(outline.selected_position(), None);
    outline.select_row(2);
    assert_eq!(outline.selected_file(), Some(&path("diario.md")));
    assert_eq!(
        outline.selected_position(),
        Some(LinePosition { line: 1, column: 0 })
    );
}

/// Otro índice deja elegido el mismo destino aunque cambie de fila; otra
/// nota vuelve a la primera, y una nota renombrada se sigue.
#[test]
fn la_eleccion_sigue_a_su_destino() {
    let mut outline = outline("diario.md");
    outline.select_row(1);
    outline.follow(&[VaultChange::Renamed {
        from: path("ideas/b.md"),
        to: path("ideas/be.md"),
        kind: EntryKind::Note,
    }]);
    outline.set_index(index(&[
        ("plan.md", "# Plan\n"),
        ("ideas/be.md", "# B\n"),
        ("diario.md", "[[be]] y el [[plan]]\n"),
    ]));
    outline.refresh();
    assert_eq!(shown(&outline), ["ideas/be.md (1) 1:0", "plan.md (1) 1:12"]);
    assert_eq!(outline.selected_row(), Some(0));
    outline.show_note(Some(path("plan.md")));
    outline.refresh();
    assert_eq!(outline.selected_row(), None);
}

/// Un embed lleva a su adjunto, que sale como destino y se abre.
#[test]
fn un_adjunto_tambien_es_un_destino() {
    let mut outline = OutgoingOutline::new(index(&[
        ("foto.png", ""),
        ("nota.md", "mira ![[foto.png]]\n"),
    ]));
    outline.show_note(Some(path("nota.md")));
    outline.refresh();
    assert_eq!(shown(&outline), ["foto.png (1) 1:5"]);
    assert_eq!(outline.selected_file(), Some(&path("foto.png")));
}
