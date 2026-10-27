use std::collections::HashMap;

use metalmind_vault::EntryKind;
use pretty_assertions::assert_eq;

use super::*;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

fn index(notes: &[(&str, &str)]) -> Arc<LinkIndex> {
    let texts: HashMap<VaultPath, String> = notes
        .iter()
        .map(|&(name, text)| (path(name), text.to_owned()))
        .collect();
    Arc::new(LinkIndex::build(
        texts.keys().map(|name| (name.clone(), EntryKind::Note)),
        |name| texts.get(name).cloned(),
    ))
}

/// `plan.md` recibe dos links de `diario.md` y uno de `ideas/b.md`.
fn vault() -> Arc<LinkIndex> {
    index(&[
        ("plan.md", "# Plan\n"),
        (
            "diario.md",
            "lunes: ver el [[plan]]\nnada\nmartes: el [[plan#Meta]] otra vez\n",
        ),
        ("ideas/b.md", "Una idea para el [[plan]].\n"),
        ("suelta.md", "sin links\n"),
    ])
}

fn outline(note: &str) -> BacklinkOutline {
    let mut outline = BacklinkOutline::new(vault());
    outline.show_note(Some(path(note)));
    outline.refresh();
    outline
}

/// Cada fila como se ve: la nota con su conteo, o el link con su contexto.
fn shown(outline: &BacklinkOutline) -> Vec<String> {
    outline
        .rows()
        .iter()
        .map(|row| match row {
            Row::Note {
                source,
                count,
                expanded,
                ..
            } => {
                let mark = if *expanded { "▾" } else { "▸" };
                format!("{mark} {} ({count})", source.as_str())
            }
            Row::Link { at, context, .. } => {
                format!("  {}:{} {}", at.line + 1, at.column, context.text)
            }
        })
        .collect()
}

#[test]
fn agrupados_por_nota_con_su_linea() {
    assert_eq!(
        shown(&outline("plan.md")),
        [
            "▾ diario.md (2)",
            "  1:14 lunes: ver el [[plan]]",
            "  3:11 martes: el [[plan#Meta]] otra vez",
            "▾ ideas/b.md (1)",
            "  1:17 Una idea para el [[plan]].",
        ]
    );
    assert_eq!(outline("plan.md").selected_row(), Some(0));
}

#[test]
fn sin_backlinks_o_sin_nota_no_hay_filas() {
    assert_eq!(outline("suelta.md").rows(), []);
    let mut none = BacklinkOutline::new(vault());
    none.show_note(None);
    none.refresh();
    assert_eq!(none.rows(), []);
    assert_eq!(none.selected_row(), None);
}

/// `j`/`k` por las filas; `enter` en un link abre su nota en el link, y en
/// una nota, en su primer link.
#[test]
fn cada_fila_abre_su_nota_donde_esta_el_link() {
    let mut outline = outline("plan.md");
    let at = |line, column| Some(LinePosition { line, column });
    assert_eq!(outline.selected_file(), Some(&path("diario.md")));
    assert_eq!(outline.selected_position(), at(0, 14));
    outline.select_next();
    outline.select_next();
    assert_eq!(outline.selected_position(), at(2, 11));
    outline.select_last();
    assert_eq!(outline.selected_file(), Some(&path("ideas/b.md")));
    assert_eq!(outline.selected_position(), at(0, 17));
}

/// `h` sube del link a su nota y la cierra; `l` la abre y baja al primero.
#[test]
fn h_y_l_pliegan_y_bajan() {
    let mut outline = outline("plan.md");
    outline.select_row(2);
    outline.collapse_or_parent();
    assert_eq!(outline.selected_row(), Some(0));
    outline.collapse_or_parent();
    assert_eq!(
        shown(&outline),
        [
            "▸ diario.md (2)",
            "▾ ideas/b.md (1)",
            "  1:17 Una idea para el [[plan]].",
        ]
    );
    outline.expand_or_enter();
    assert_eq!(outline.rows().len(), 5);
    assert_eq!(outline.selected_row(), Some(0));
    outline.expand_or_enter();
    assert_eq!(outline.selected_row(), Some(1));
}

/// Otro índice deja la fila elegida si sigue; otra nota vuelve a la primera.
#[test]
fn otro_indice_deja_la_eleccion_y_otra_nota_la_reinicia() {
    let mut outline = outline("plan.md");
    outline.select_row(4);
    outline.set_index(index(&[
        ("plan.md", "# Plan\n"),
        ("diario.md", "lunes: ver el [[plan]]\n"),
        ("ideas/b.md", "Una idea para el [[plan]].\n"),
    ]));
    outline.refresh();
    assert_eq!(outline.rows().len(), 4);
    assert_eq!(outline.selected_row(), Some(3));
    assert_eq!(outline.selected_file(), Some(&path("ideas/b.md")));
    outline.show_note(Some(path("diario.md")));
    outline.refresh();
    assert_eq!(outline.selected_row(), None);
}

/// La nota activa renombrada sigue mostrándose con su ruta nueva.
#[test]
fn sigue_a_la_nota_renombrada() {
    let mut outline = outline("plan.md");
    outline.follow(&[VaultChange::Renamed {
        from: path("plan.md"),
        to: path("proyectos/plan.md"),
        kind: EntryKind::Note,
    }]);
    assert_eq!(outline.note(), Some(&path("proyectos/plan.md")));
}

/// Una nota cerrada sigue cerrada con su ruta nueva, también si se mueve su
/// carpeta.
#[test]
fn una_nota_cerrada_renombrada_sigue_cerrada() {
    let mut outline = outline("plan.md");
    outline.select_row(3);
    outline.collapse_or_parent();
    let renamed = |from: &str, to: &str, kind| VaultChange::Renamed {
        from: path(from),
        to: path(to),
        kind,
    };
    let changes = [renamed("ideas", "viejas", EntryKind::Folder)];
    outline.follow(&changes);
    outline.set_index(index(&[
        ("plan.md", "# Plan\n"),
        (
            "diario.md",
            "lunes: ver el [[plan]]\nnada\nmartes: el [[plan#Meta]] otra vez\n",
        ),
        ("viejas/b.md", "Una idea para el [[plan]].\n"),
    ]));
    outline.refresh();
    assert_eq!(
        shown(&outline),
        [
            "▾ diario.md (2)",
            "  1:14 lunes: ver el [[plan]]",
            "  3:11 martes: el [[plan#Meta]] otra vez",
            "▸ viejas/b.md (1)",
        ]
    );
    assert_eq!(outline.selected_file(), Some(&path("viejas/b.md")));
}

/// Lo que mira el menú de un backlink para seguir abierto: la fila elegida
/// cambia si su link desaparece, no si solo se renombra su nota.
#[test]
fn la_eleccion_cambia_solo_si_su_link_ya_no_esta() {
    let mut outline = outline("plan.md");
    outline.select_row(2);
    let chosen = outline.selected_id().cloned();
    outline.set_index(index(&[
        ("plan.md", "# Plan\n"),
        ("diario.md", "lunes: ver el [[plan]]\n"),
        ("ideas/b.md", "Una idea para el [[plan]].\n"),
    ]));
    outline.refresh();
    assert_ne!(outline.selected_id(), chosen.as_ref());
    outline.select_row(3);
    let chosen = outline.selected_id().cloned();
    outline.follow(&[VaultChange::Renamed {
        from: path("ideas/b.md"),
        to: path("ideas/c.md"),
        kind: EntryKind::Note,
    }]);
    outline.set_index(index(&[
        ("plan.md", "# Plan\n"),
        ("diario.md", "lunes: ver el [[plan]]\n"),
        ("ideas/c.md", "Una idea para el [[plan]].\n"),
    ]));
    let renamed = outline.selected_id().cloned();
    outline.refresh();
    assert_ne!(renamed, chosen, "la ruta cambió");
    assert_eq!(
        outline.selected_id(),
        renamed.as_ref(),
        "pero es la misma fila"
    );
}
