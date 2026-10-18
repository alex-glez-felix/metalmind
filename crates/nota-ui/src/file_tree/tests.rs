use std::sync::Arc;

use nota_vault::{EntryKind, VaultChange, VaultIndex, VaultPath};
use pretty_assertions::assert_eq;

use super::FileTree;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Un índice con las rutas dadas; las que terminan en `/` son carpetas.
fn index(paths: &[&str]) -> Arc<VaultIndex> {
    Arc::new(VaultIndex::from_entries(paths.iter().map(
        |&p| match p.strip_suffix('/') {
            Some(folder) => (path(folder), EntryKind::Folder),
            None if p.to_ascii_lowercase().ends_with(".md") => (path(p), EntryKind::Note),
            None => (path(p), EntryKind::Attachment),
        },
    )))
}

const VAULT: &[&str] = &[
    "diario/",
    "diario/2026/",
    "diario/2026/enero.md",
    "diario/hoy.md",
    "proyectos/",
    "proyectos/nota.md",
    "proyectos/plano.png",
    "a.md",
    "b.md",
];

/// Las filas como se ven: sangría de dos espacios por nivel, `▾`/`▸` en
/// las carpetas abiertas o cerradas y `*` en la seleccionada.
fn render(tree: &FileTree) -> Vec<String> {
    tree.rows()
        .iter()
        .enumerate()
        .map(|(at, row)| {
            let marker = match (row.is_folder(), row.expanded) {
                (true, true) => "▾ ",
                (true, false) => "▸ ",
                (false, _) => "",
            };
            let selected = if tree.selected_row() == Some(at) {
                " *"
            } else {
                ""
            };
            format!("{}{marker}{}{selected}", "  ".repeat(row.depth), row.name())
        })
        .collect()
}

#[test]
fn empieza_cerrado_en_el_orden_del_indice() {
    let tree = FileTree::new(index(VAULT));
    assert_eq!(render(&tree), ["▸ diario *", "▸ proyectos", "a.md", "b.md"]);
}

#[test]
fn un_vault_vacio_no_tiene_seleccion() {
    let mut tree = FileTree::new(index(&[]));
    tree.select_next();
    tree.collapse_or_parent();
    tree.expand_or_enter();
    assert_eq!(tree.rows(), []);
    assert_eq!(tree.selected(), None);
}

#[test]
fn expandir_y_colapsar() {
    let mut tree = FileTree::new(index(VAULT));
    tree.expand(&path("diario"));
    tree.expand(&path("diario/2026"));
    assert_eq!(
        render(&tree),
        [
            "▾ diario *",
            "  ▾ 2026",
            "    enero.md",
            "  hoy.md",
            "▸ proyectos",
            "a.md",
            "b.md"
        ]
    );
    tree.collapse(&path("diario"));
    assert_eq!(render(&tree), ["▸ diario *", "▸ proyectos", "a.md", "b.md"]);
    tree.toggle(&path("diario"));
    assert_eq!(render(&tree)[1], "  ▾ 2026", "recuerda lo abierto dentro");
    tree.expand(&path("a.md"));
    assert_eq!(tree.rows().len(), 7, "un archivo no se expande");
}

#[test]
fn moverse_no_da_la_vuelta() {
    let mut tree = FileTree::new(index(VAULT));
    tree.select_previous();
    assert_eq!(tree.selected(), Some(&path("diario")));
    tree.select_next();
    tree.select_next();
    assert_eq!(tree.selected(), Some(&path("a.md")));
    tree.select_last();
    tree.select_next();
    assert_eq!(tree.selected(), Some(&path("b.md")));
    tree.select_first();
    assert_eq!(tree.selected_row(), Some(0));
}

#[test]
fn h_y_l_como_en_los_exploradores_de_nvim() {
    let mut tree = FileTree::new(index(VAULT));
    tree.expand_or_enter();
    assert_eq!(render(&tree)[..3], ["▾ diario *", "  ▸ 2026", "  hoy.md"]);
    tree.expand_or_enter();
    assert_eq!(tree.selected(), Some(&path("diario/2026")));
    tree.expand_or_enter();
    tree.expand_or_enter();
    assert_eq!(tree.selected(), Some(&path("diario/2026/enero.md")));
    tree.expand_or_enter();
    assert_eq!(tree.selected(), Some(&path("diario/2026/enero.md")));
    tree.collapse_or_parent();
    assert_eq!(tree.selected(), Some(&path("diario/2026")));
    tree.collapse_or_parent();
    assert_eq!(render(&tree)[1], "  ▸ 2026 *");
    tree.collapse_or_parent();
    assert_eq!(tree.selected(), Some(&path("diario")));
    tree.collapse_or_parent();
    tree.collapse_or_parent();
    assert_eq!(render(&tree), ["▸ diario *", "▸ proyectos", "a.md", "b.md"]);
}

#[test]
fn seleccionar_abre_las_carpetas_que_lo_contienen() {
    let mut tree = FileTree::new(index(VAULT));
    assert!(tree.select(&path("diario/2026/enero.md")));
    assert_eq!(render(&tree)[2], "    enero.md *");
    assert!(!tree.select(&path("no/existe.md")));
    assert!(!tree.select(&VaultPath::root()));
}

#[test]
fn el_filtro_muestra_las_coincidencias_y_sus_carpetas() {
    let mut tree = FileTree::new(index(VAULT));
    tree.set_filter("enero");
    assert_eq!(render(&tree), ["▾ diario *", "  ▾ 2026", "    enero.md"]);
    assert_eq!(tree.selected_row(), Some(0), "la fila de la selección");
    assert_eq!(tree.rows()[2].highlights, [0, 1, 2, 3, 4]);
    assert_eq!(tree.rows()[0].highlights, Vec::<usize>::new());
    tree.toggle(&path("diario"));
    assert_eq!(tree.rows().len(), 3, "con filtro todo se ve abierto");
    tree.set_filter("");
    assert_eq!(render(&tree), ["▸ diario *", "▸ proyectos", "a.md", "b.md"]);
}

#[test]
fn el_filtro_conserva_la_seleccion_si_se_ve() {
    let mut tree = FileTree::new(index(VAULT));
    tree.select(&path("proyectos/nota.md"));
    tree.set_filter("nota");
    assert_eq!(tree.selected(), Some(&path("proyectos/nota.md")));
    tree.set_filter("zzz");
    assert_eq!(tree.rows(), []);
    assert_eq!(tree.selected(), None);
}

#[test]
fn renombrar_lleva_la_seleccion_y_lo_abierto() {
    let mut tree = FileTree::new(index(VAULT));
    tree.select(&path("diario/2026/enero.md"));
    let renamed = index(&[
        "notas/",
        "notas/2026/",
        "notas/2026/enero.md",
        "notas/hoy.md",
        "proyectos/",
        "proyectos/nota.md",
        "proyectos/plano.png",
        "a.md",
        "b.md",
    ]);
    let change = VaultChange::Renamed {
        from: path("diario"),
        to: path("notas"),
        kind: EntryKind::Folder,
    };
    tree.apply(&[change], renamed);
    assert_eq!(tree.selected(), Some(&path("notas/2026/enero.md")));
    assert_eq!(
        render(&tree)[..4],
        ["▾ notas", "  ▾ 2026", "    enero.md *", "  hoy.md"]
    );
}

#[test]
fn si_lo_seleccionado_desaparece_queda_la_fila_de_su_lugar() {
    let mut tree = FileTree::new(index(VAULT));
    tree.select(&path("a.md"));
    let change = VaultChange::Removed {
        path: path("a.md"),
        kind: EntryKind::Note,
    };
    let without: Vec<&str> = VAULT.iter().copied().filter(|p| *p != "a.md").collect();
    tree.apply(&[change], index(&without));
    assert_eq!(tree.selected(), Some(&path("b.md")));
    let change = VaultChange::Removed {
        path: path("b.md"),
        kind: EntryKind::Note,
    };
    let without: Vec<&str> = without.into_iter().filter(|p| *p != "b.md").collect();
    tree.apply(&[change], index(&without));
    assert_eq!(tree.selected(), Some(&path("proyectos")));
}

#[test]
fn borrar_una_carpeta_abierta_la_olvida() {
    let mut tree = FileTree::new(index(VAULT));
    tree.expand(&path("diario"));
    let without: Vec<&str> = VAULT
        .iter()
        .copied()
        .filter(|p| !p.starts_with("diario"))
        .collect();
    let removed = VaultChange::Removed {
        path: path("diario"),
        kind: EntryKind::Folder,
    };
    tree.apply(&[removed], index(&without));
    assert_eq!(tree.expanded().count(), 0);
    let created = VaultChange::Created {
        path: path("diario"),
        kind: EntryKind::Folder,
    };
    tree.apply(&[created], index(VAULT));
    assert_eq!(render(&tree)[0], "▸ diario", "vuelve cerrada");
}

#[test]
fn revelar_espera_a_que_el_watcher_lo_traiga() {
    let mut tree = FileTree::new(index(VAULT));
    tree.reveal(path("proyectos/nueva.md"));
    assert_eq!(tree.selected(), Some(&path("diario")));
    let mut with: Vec<&str> = VAULT.to_vec();
    with.push("proyectos/nueva.md");
    let change = VaultChange::Created {
        path: path("proyectos/nueva.md"),
        kind: EntryKind::Note,
    };
    tree.apply(&[change], index(&with));
    assert_eq!(tree.selected(), Some(&path("proyectos/nueva.md")));
    assert_eq!(render(&tree)[3], "  nueva.md *");
}

#[test]
fn lo_abierto_de_la_sesion_anterior_que_ya_no_existe_se_ignora() {
    let tree = FileTree::with_expanded(
        index(VAULT),
        [path("diario"), path("a.md"), path("no-existe")],
    );
    assert_eq!(tree.expanded().collect::<Vec<_>>(), [&path("diario")]);
}

/// Una nota creada con el filtro puesto queda seleccionada si el filtro la
/// muestra; si no, en cuanto se quita el filtro.
#[test]
fn revelar_con_un_filtro_puesto() {
    let mut with: Vec<&str> = VAULT.to_vec();
    with.push("proyectos/nueva.md");
    let created = || VaultChange::Created {
        path: path("proyectos/nueva.md"),
        kind: EntryKind::Note,
    };

    let mut tree = FileTree::new(index(VAULT));
    tree.set_filter("nueva");
    tree.reveal(path("proyectos/nueva.md"));
    tree.apply(&[created()], index(&with));
    assert_eq!(tree.selected(), Some(&path("proyectos/nueva.md")));

    let mut tree = FileTree::new(index(VAULT));
    tree.set_filter("enero");
    tree.reveal(path("proyectos/nueva.md"));
    tree.apply(&[created()], index(&with));
    assert_ne!(tree.selected(), Some(&path("proyectos/nueva.md")));
    tree.set_filter("");
    assert_eq!(tree.selected(), Some(&path("proyectos/nueva.md")));
}
