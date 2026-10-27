//! Llevar los tags al día con los cambios del watcher, sin reescanear lo que
//! no cambió.

use std::collections::{BTreeMap, BTreeSet};

use crate::index::{EntryKind, VaultIndex};
use crate::path::VaultPath;
use crate::refresh::VaultChange;
use crate::tags::graph::TagGraph;
use crate::tags::key::TagKey;
use crate::tags::note::page_name;
use crate::tags::{ReadNote, Tags, scan_one};

/// Cuánto hay que mirar de nuevo una ruta al final del lote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Settle {
    /// Llegó por un rename: solo se escanea si no estaba en el índice.
    IfMissing,
    /// Su contenido cambió, o pasó a ser o dejó de ser página.
    Rescan,
}

impl Tags {
    /// Aplica los cambios de un lote, con `vault` ya actualizado, y devuelve
    /// las claves cuyo resultado puede haber cambiado. Una nota renombrada
    /// conserva sus tags sin releerse, salvo que entre o salga de la carpeta
    /// de tags.
    pub(crate) fn apply(
        &mut self,
        changes: &[VaultChange],
        vault: &VaultIndex,
        read: &impl ReadNote,
    ) -> Vec<TagKey> {
        let mut pages_changed = false;
        let mut touched = BTreeSet::new();
        let mut settle: BTreeMap<VaultPath, Settle> = BTreeMap::new();
        let mark = |settle: &mut BTreeMap<VaultPath, Settle>, path: VaultPath, how: Settle| {
            let entry = settle.entry(path).or_insert(how);
            *entry = (*entry).max(how);
        };
        for change in changes {
            match change {
                VaultChange::Created { path, .. } | VaultChange::Modified { path } => {
                    mark(&mut settle, path.clone(), Settle::Rescan);
                }
                VaultChange::Removed { path, kind } => {
                    for note in self.indexed_under(path, *kind) {
                        touched.extend(self.forget(&note, &mut pages_changed));
                    }
                }
                VaultChange::Renamed { from, to, kind } => {
                    for old in self.indexed_under(from, *kind) {
                        let Some(new) = old.rebase(from, to) else {
                            continue;
                        };
                        let how = self.moved(&old, &new, &mut touched, &mut pages_changed);
                        mark(&mut settle, new, how);
                    }
                    for note in notes_under(vault, to) {
                        mark(&mut settle, note, Settle::IfMissing);
                    }
                }
            }
        }
        for (path, how) in settle {
            touched.extend(self.settle(&path, how, vault, read, &mut pages_changed));
        }
        if pages_changed {
            let graph = self.rebuild_graph();
            touched.extend(graph_keys(&self.graph));
            touched.extend(graph_keys(&graph));
            self.graph = graph;
        }
        touched.into_iter().collect()
    }

    /// Las notas indexadas que son `path` o, si es carpeta, están dentro de él.
    fn indexed_under(&self, path: &VaultPath, kind: EntryKind) -> Vec<VaultPath> {
        match kind {
            EntryKind::Note => return vec![path.clone()],
            EntryKind::Attachment => return Vec::new(),
            EntryKind::Folder => {}
        }
        self.index
            .all()
            .map(|(note, _)| note)
            .filter(|note| note.starts_with(path))
            .cloned()
            .collect()
    }

    fn forget(&mut self, path: &VaultPath, pages_changed: &mut bool) -> BTreeSet<TagKey> {
        *pages_changed |= self.drop_page(path);
        self.index.remove(path)
    }

    /// Mueve las ocurrencias de `old` a `new`. Si cambia si es página, hay
    /// que releerla.
    fn moved(
        &mut self,
        old: &VaultPath,
        new: &VaultPath,
        touched: &mut BTreeSet<TagKey>,
        pages_changed: &mut bool,
    ) -> Settle {
        let was_page = self.drop_page(old);
        *pages_changed |= was_page;
        let Some((note, keys)) = self.index.take(old) else {
            return Settle::IfMissing;
        };
        touched.extend(keys);
        if was_page || page_name(&self.folder, new).is_some() {
            return Settle::Rescan;
        }
        touched.extend(self.index.insert(new.clone(), note));
        Settle::IfMissing
    }

    fn settle(
        &mut self,
        path: &VaultPath,
        how: Settle,
        vault: &VaultIndex,
        read: &impl ReadNote,
        pages_changed: &mut bool,
    ) -> BTreeSet<TagKey> {
        if vault.kind(path) != Some(EntryKind::Note) {
            return self.forget(path, pages_changed);
        }
        if how == Settle::IfMissing && self.index.note(path).is_some() {
            return BTreeSet::new();
        }
        let (note, page) = scan_one(&self.folder, path, read);
        let (touched, graph_stale) = self.store(path.clone(), note, page);
        *pages_changed |= graph_stale;
        touched
    }
}

fn notes_under(vault: &VaultIndex, path: &VaultPath) -> Vec<VaultPath> {
    vault
        .subtree(path)
        .into_iter()
        .filter(|(_, kind)| *kind == EntryKind::Note)
        .map(|(note, _)| note)
        .collect()
}

/// Lo que nombra el grafo: páginas, alias y padres.
fn graph_keys(graph: &TagGraph) -> BTreeSet<TagKey> {
    let mut keys = BTreeSet::new();
    for page in graph.pages() {
        keys.insert(page.key.clone());
        keys.extend(page.aliases.iter().map(|alias| alias.key.clone()));
        keys.extend(page.parents.iter().map(|parent| parent.key.clone()));
    }
    keys
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use proptest::prelude::*;
    use tempfile::TempDir;

    use super::*;
    use crate::refresh::{Touched, refresh};
    use crate::scan::scan;
    use crate::tags::DEFAULT_TAGS_FOLDER;

    #[derive(Debug, Clone)]
    enum Op {
        Write(String, &'static str),
        CreateDir(String),
        Remove(String),
        Rename(String, String),
    }

    /// Contenidos que cambian tags, alias y padres, para que crear, mover y
    /// editar notas y páginas mueva el índice y el grafo.
    const CONTENTS: &[&str] = &[
        "",
        "#rust",
        "#Rust #gpui",
        "---\ntags: [rs, nota]\n---\n#a",
        "---\naliases: [rs]\nparents: [lenguajes]\n---\n#rust",
        "---\nparents: [rust, a]\n---\n",
        "---\nparents: rs\ncolor: rojo\n---\n#b",
        "---\ntítulo: Algo\nfecha: [2024, {mes: 10}]\n---\n",
    ];

    fn op() -> impl Strategy<Value = Op> {
        let path = prop::collection::vec(
            prop::sample::select(vec!["tags", "a", "rust.md", "rs.md", "a.md", "x y.md"]),
            1..=2,
        )
        .prop_map(|parts| parts.join("/"));
        prop_oneof![
            (path.clone(), prop::sample::select(CONTENTS))
                .prop_map(|(path, text)| Op::Write(path, text)),
            path.clone().prop_map(Op::CreateDir),
            path.clone().prop_map(Op::Remove),
            (path.clone(), path).prop_map(|(from, to)| Op::Rename(from, to)),
        ]
    }

    /// Aplica `op` en disco y devuelve lo que el watcher diría que tocó.
    fn apply(root: &Path, op: &Op) -> Touched {
        let at = |path: &str| root.join(path);
        let vault = |path: &str| VaultPath::new(path).unwrap();
        let mut touched = Touched::default();
        let _ = match op {
            Op::Write(path, text) => {
                let _ = fs::create_dir_all(at(path).parent().unwrap());
                touched.paths.insert(vault(path));
                touched.written.insert(vault(path));
                fs::write(at(path), text)
            }
            Op::CreateDir(path) => {
                touched.paths.insert(vault(path));
                fs::create_dir_all(at(path))
            }
            Op::Remove(path) => {
                touched.paths.insert(vault(path));
                fs::remove_file(at(path)).or_else(|_| fs::remove_dir_all(at(path)))
            }
            Op::Rename(from, to) => {
                let _ = fs::create_dir_all(at(to).parent().unwrap());
                touched.paths.extend([vault(from), vault(to)]);
                touched.renames.push((vault(from), vault(to)));
                fs::rename(at(from), at(to))
            }
        };
        touched
    }

    /// Aplica cada lote en disco y en los tags, y compara con armarlos de cero.
    fn check(batches: &[Vec<Op>]) -> Result<(), TestCaseError> {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let folder = VaultPath::fixed(DEFAULT_TAGS_FOLDER);
        let read = |path: &VaultPath| fs::read_to_string(path.to_fs(&root)).ok();
        let mut index = scan(&root, None);
        let mut tags = Tags::build(folder.clone(), &index, read);
        for batch in batches {
            let mut touched = Touched::default();
            for op in batch {
                touched.merge(apply(&root, op));
            }
            let changes = refresh(&mut index, &root, &touched, None);
            tags.apply(&changes, &index, &read);
            let full = Tags::build(folder.clone(), &scan(&root, None), read);
            prop_assert_eq!(&tags, &full, "tras {:?}: {:?}", batch, changes);
        }
        Ok(())
    }

    /// Un rename pisa una nota ya indexada y después se mueve su carpeta, en
    /// el mismo lote: la nota se relee. Lo encontró el proptest de abajo.
    #[test]
    fn pisar_con_un_rename_y_mover_la_carpeta() {
        check(&[
            vec![Op::Write("a/rust.md".into(), "#rust")],
            vec![Op::CreateDir("a".into())],
            vec![
                Op::Write("tags".into(), "#Rust #gpui"),
                Op::Rename("tags".into(), "a/rust.md".into()),
                Op::Rename("a".into(), "rust.md".into()),
            ],
        ])
        .unwrap();
    }

    /// Dos renames del lote llegan a la misma nota: queda el contenido del
    /// segundo, sin tags. Lo encontró el proptest de abajo.
    #[test]
    fn dos_renames_al_mismo_destino() {
        check(&[
            vec![Op::Write("rust.md/rust.md".into(), "#Rust #gpui")],
            vec![Op::Write("tags".into(), "")],
            vec![
                Op::Rename("rust.md/rust.md".into(), "a.md".into()),
                Op::Rename("tags".into(), "a.md".into()),
            ],
        ])
        .unwrap();
    }

    /// Se borra la carpeta de tags y otra toma su nombre en el mismo lote:
    /// la página que queda en la misma ruta es otra y se relee. Lo encontró
    /// el proptest de abajo.
    #[test]
    fn otra_carpeta_toma_el_lugar_de_la_borrada() {
        check(&[
            vec![Op::Write("tags/rust.md".into(), "")],
            vec![Op::Write("rs.md/rust.md".into(), "#rust")],
            vec![
                Op::Remove("tags".into()),
                Op::Rename("rs.md".into(), "tags".into()),
            ],
        ])
        .unwrap();
    }

    /// Dos carpetas se cruzan en un lote: la nota que queda en `a/rs.md` es
    /// otra y se relee (#70).
    #[test]
    fn dos_carpetas_se_cruzan() {
        check(&[
            vec![Op::Write("a/rs.md".into(), "")],
            vec![Op::Write("rs.md/rs.md".into(), "#rust")],
            vec![
                Op::Rename("a".into(), "rust.md".into()),
                Op::Rename("rs.md".into(), "a".into()),
            ],
        ])
        .unwrap();
    }

    /// Una nota pisa a otra que acaba de llegar con su carpeta (#70).
    #[test]
    fn pisar_una_nota_que_llego_con_su_carpeta() {
        check(&[
            vec![
                Op::Write("x y.md".into(), ""),
                Op::Write("rust.md/a.md".into(), "#rust"),
            ],
            vec![
                Op::Rename("rust.md".into(), "rs.md".into()),
                Op::Rename("x y.md".into(), "rs.md/a.md".into()),
            ],
        ])
        .unwrap();
    }

    /// Se borra una carpeta y otra toma su nombre: sus notas se releen (#71).
    #[test]
    fn borrar_una_carpeta_y_renombrar_otra_encima() {
        check(&[
            vec![Op::Write("a/x y.md".into(), "")],
            vec![Op::CreateDir("rs.md/tags".into())],
            vec![Op::Write("x y.md/x y.md".into(), "#rust")],
            vec![
                Op::Remove("x y.md".into()),
                Op::Rename("a".into(), "x y.md".into()),
            ],
        ])
        .unwrap();
    }

    /// Dos carpetas se cruzan y una es la de tags (#82).
    #[test]
    fn la_carpeta_de_tags_se_cruza_con_otra() {
        check(&[
            vec![Op::Write("tags/a.md".into(), "#rust")],
            vec![Op::Write("a.md/a.md".into(), "#Rust #gpui")],
            vec![
                Op::Rename("a.md".into(), "a".into()),
                Op::Rename("tags".into(), "a.md".into()),
            ],
        ])
        .unwrap();
    }

    /// Dos renames al mismo destino, el segundo de una nota escrita en el
    /// mismo lote: queda su contenido (#103).
    #[test]
    fn dos_renames_al_mismo_destino_con_una_nota_nueva() {
        check(&[
            vec![Op::Write("rs.md".into(), "")],
            vec![
                Op::Rename("rs.md".into(), "x y.md".into()),
                Op::Write("rust.md".into(), "#rust"),
                Op::Rename("rust.md".into(), "x y.md".into()),
            ],
        ])
        .unwrap();
    }

    /// El frontmatter entero sigue a la nota: cambia al editarla y se va con
    /// ella al renombrarla, sin releerla.
    #[test]
    fn el_frontmatter_sigue_a_la_nota() {
        use metalmind_syntax::{Property, PropertyValue};
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let read = |path: &VaultPath| fs::read_to_string(path.to_fs(&root)).ok();
        let title = |text: &str| {
            vec![Property {
                key: "título".to_owned(),
                value: PropertyValue::Text(text.to_owned()),
            }]
        };
        fs::write(root.join("a.md"), "---\ntítulo: Uno\n---\n").unwrap();
        let mut index = scan(&root, None);
        let mut tags = Tags::build(VaultPath::fixed(DEFAULT_TAGS_FOLDER), &index, read);
        let a = VaultPath::new("a.md").unwrap();
        assert_eq!(tags.index().frontmatter(&a), title("Uno"));

        let mut batch = |op: Op, tags: &mut Tags| {
            let changes = refresh(&mut index, &root, &apply(&root, &op), None);
            tags.apply(&changes, &index, &read);
        };
        batch(
            Op::Write("a.md".into(), "---\ntítulo: Dos\n---\n"),
            &mut tags,
        );
        assert_eq!(tags.index().frontmatter(&a), title("Dos"));
        batch(Op::Rename("a.md".into(), "b.md".into()), &mut tags);
        assert_eq!(tags.index().frontmatter(&a), []);
        let b = VaultPath::new("b.md").unwrap();
        assert_eq!(tags.index().frontmatter(&b), title("Dos"));
        batch(Op::Write("b.md".into(), "sin frontmatter"), &mut tags);
        assert_eq!(tags.index().frontmatter(&b), []);
    }

    proptest! {
        /// Índice y grafo incrementales tras cualquier secuencia de crear,
        /// editar, mover y borrar notas y páginas == construidos de cero
        /// (05 §2).
        #[test]
        fn incremental_igual_a_completo(
            batches in prop::collection::vec(prop::collection::vec(op(), 1..5), 1..12),
        ) {
            check(&batches)?;
        }
    }
}
