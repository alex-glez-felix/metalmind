use std::fs;

use metalmind_vault::Vault;
use pretty_assertions::assert_eq;

use super::*;

const FILES: &[(&str, &str)] = &[
    ("parser.md", "#Rust y el parser"),
    ("viejo.md", "#Rust también"),
    ("gpui.md", "---\ntags: [gpui]\n---\n"),
    ("ui.md", "#ui"),
    ("sin tags.md", "Nada."),
    ("Otra sin tags.md", "Tampoco."),
    ("tags/gpui.md", "---\nparents: [rust, ui]\n---\n"),
];

/// Ya armado, como al mostrar la vista de tags.
fn outline_of(tags: Arc<Tags>) -> TagOutline {
    let mut outline = TagOutline::new(tags);
    outline.refresh();
    outline
}

fn tags_of(files: &[(&str, &str)]) -> Arc<Tags> {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let (vault, _updates) = Vault::open(dir.path()).unwrap();
    vault.tags()
}

/// Cada fila como se ve: sangría, triángulo, nombre y conteo; `>` marca la
/// seleccionada.
fn shown(outline: &TagOutline) -> Vec<String> {
    outline
        .rows()
        .iter()
        .enumerate()
        .map(|(at, row)| {
            let cursor = if outline.selected_row() == Some(at) {
                ">"
            } else {
                " "
            };
            let indent = "  ".repeat(row.depth);
            let label = match &row.kind {
                RowKind::Tag {
                    name,
                    count,
                    expandable,
                    expanded,
                    cycle,
                    ..
                } => {
                    let arrow = match (expandable, expanded) {
                        (_, true) => "▾ ",
                        (true, false) => "▸ ",
                        (false, false) => "  ",
                    };
                    let cycle = if *cycle { " ↻" } else { "" };
                    format!("{arrow}#{name} ({count}){cycle}")
                }
                RowKind::Note => format!("  {}", row.note().unwrap().name()),
                RowKind::Inbox { count, expanded } => {
                    let arrow = if *expanded { "▾ " } else { "▸ " };
                    format!("{arrow}Buzón ({count})")
                }
            };
            format!("{cursor}{indent}{label}")
        })
        .collect()
}

#[test]
fn abre_cerrado_con_las_raices_y_el_buzon() {
    let outline = outline_of(tags_of(FILES));
    assert_eq!(
        shown(&outline),
        [">▸ #Rust (3)", " ▸ #ui (2)", " ▸ Buzón (2)"]
    );
}

/// Bajo un tag, primero sus tags hijos y luego sus notas (05 §3).
#[test]
fn l_abre_y_baja_y_h_sube_y_cierra() {
    let mut outline = outline_of(tags_of(FILES));
    outline.expand_or_enter();
    assert_eq!(
        shown(&outline),
        [
            ">▾ #Rust (3)",
            "   ▸ #gpui (1)",
            "     parser.md",
            "     viejo.md",
            " ▸ #ui (2)",
            " ▸ Buzón (2)",
        ]
    );
    outline.expand_or_enter();
    assert_eq!(outline.selected_row(), Some(1));
    outline.select_next();
    outline.collapse_or_parent();
    assert_eq!(outline.selected_row(), Some(0));
    outline.collapse_or_parent();
    assert_eq!(
        shown(&outline),
        [">▸ #Rust (3)", " ▸ #ui (2)", " ▸ Buzón (2)"]
    );
}

/// `#gpui` tiene dos padres: sale bajo los dos, y abrirlo bajo uno no lo
/// abre bajo el otro.
#[test]
fn un_tag_con_dos_padres_se_abre_en_cada_lugar_por_separado() {
    let mut outline = outline_of(tags_of(FILES));
    outline.expand_or_enter();
    outline.select_row(4);
    outline.expand_or_enter();
    outline.select_row(1);
    outline.expand_or_enter();
    assert_eq!(
        shown(&outline),
        [
            " ▾ #Rust (3)",
            ">  ▾ #gpui (1)",
            "       gpui.md",
            "     parser.md",
            "     viejo.md",
            " ▾ #ui (2)",
            "   ▸ #gpui (1)",
            "     ui.md",
            " ▸ Buzón (2)",
        ]
    );
}

#[test]
fn el_buzon_tiene_las_notas_sin_tags_en_orden() {
    let mut outline = outline_of(tags_of(FILES));
    outline.select_last();
    outline.toggle_selected();
    assert_eq!(
        shown(&outline)[2..],
        [">▾ Buzón (2)", "     Otra sin tags.md", "     sin tags.md"]
    );
}

/// Un lote del watcher cambia los conteos sin cerrar nada ni mover la
/// selección de su fila.
#[test]
fn otro_lote_conserva_lo_abierto_y_la_seleccion() {
    let tags = tags_of(FILES);
    let mut outline = outline_of(Arc::clone(&tags));
    outline.expand_or_enter();
    outline.select_row(3);
    let nueva = VaultPath::new("a nueva.md").unwrap();
    outline.set_tags(Arc::new(tags.with_unsaved([(&nueva, "#rust")])));
    outline.refresh();
    assert_eq!(
        shown(&outline),
        [
            " ▾ #Rust (4)",
            "   ▸ #gpui (1)",
            "     a nueva.md",
            "     parser.md",
            ">    viejo.md",
            " ▸ #ui (2)",
            " ▸ Buzón (2)",
        ]
    );
}

/// Si la fila seleccionada desaparece, queda la que ocupaba su lugar.
#[test]
fn si_la_seleccionada_desaparece_queda_su_lugar() {
    let tags = tags_of(FILES);
    let mut outline = outline_of(Arc::clone(&tags));
    outline.expand_or_enter();
    outline.select_row(2);
    let parser = VaultPath::new("parser.md").unwrap();
    outline.set_tags(Arc::new(tags.with_unsaved([(&parser, "sin tags ya")])));
    outline.refresh();
    assert_eq!(shown(&outline)[2], ">    viejo.md");
}

/// `a` y `b` son padres el uno del otro bajo `c`: la segunda vez que
/// aparece `a` en el camino se marca y no se abre.
#[test]
fn un_ciclo_no_se_abre_dos_veces() {
    let mut outline = outline_of(tags_of(&[
        ("nota.md", "#a #b #c"),
        ("tags/a.md", "---\nparents: [b, c]\n---\n"),
        ("tags/b.md", "---\nparents: [a]\n---\n"),
    ]));
    for _ in 0..3 {
        outline.expand_or_enter();
        outline.expand_or_enter();
    }
    let tags: Vec<String> = shown(&outline)
        .into_iter()
        .filter(|line| line.contains('#'))
        .collect();
    assert_eq!(
        tags,
        [
            " ▾ #c (1)",
            "   ▾ #a (1)",
            "     ▾ #b (1)",
            ">        #a (1) ↻",
        ]
    );
}

#[test]
fn sin_tags_queda_solo_el_buzon() {
    let outline = outline_of(tags_of(&[("a.md", "nada")]));
    assert_eq!(shown(&outline), [">▸ Buzón (1)"]);
}

/// Una nota seleccionada que se renombra sigue elegida en su nuevo lugar,
/// no queda la que ocupa su fila de antes.
#[test]
fn la_nota_seleccionada_sigue_a_su_renombre() {
    let mut outline = outline_of(tags_of(FILES));
    outline.expand_or_enter();
    outline.select_row(3);
    assert_eq!(shown(&outline)[3], ">    viejo.md");
    let renamed: Vec<(&str, &str)> = FILES
        .iter()
        .map(|&(name, text)| {
            (
                if name == "viejo.md" {
                    "a viejo.md"
                } else {
                    name
                },
                text,
            )
        })
        .collect();
    outline.follow(&[VaultChange::Renamed {
        from: VaultPath::new("viejo.md").unwrap(),
        to: VaultPath::new("a viejo.md").unwrap(),
        kind: metalmind_vault::EntryKind::Note,
    }]);
    outline.set_tags(tags_of(&renamed));
    outline.refresh();
    assert_eq!(
        shown(&outline)[..4],
        [
            " ▾ #Rust (3)",
            "   ▸ #gpui (1)",
            ">    a viejo.md",
            "     parser.md"
        ]
    );
}
