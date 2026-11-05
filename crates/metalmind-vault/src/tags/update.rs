//! Llevar los tags al día con los cambios del watcher, sin reescanear lo que
//! no cambió.

use std::collections::BTreeSet;

use crate::index::VaultIndex;
use crate::note_batch::NoteBatch;
use crate::note_changes::{PerNoteIndex, Settle, apply_changes};
use crate::path::VaultPath;
use crate::refresh::VaultChange;
use crate::tags::graph::TagGraph;
use crate::tags::key::TagKey;
use crate::tags::note::page_name;
use crate::tags::{Tags, scan_in_batch};

impl Tags {
    /// Aplica los cambios de un lote, con `vault` ya actualizado, y devuelve
    /// las claves cuyo resultado puede haber cambiado. Una nota renombrada
    /// conserva sus tags sin releerse, salvo que entre o salga de la carpeta
    /// de tags.
    pub(crate) fn apply(
        &mut self,
        changes: &[VaultChange],
        vault: &VaultIndex,
        notes: &mut NoteBatch<'_>,
    ) -> Vec<TagKey> {
        let mut batch = TagsBatch {
            tags: self,
            touched: BTreeSet::new(),
            pages_changed: false,
        };
        apply_changes(&mut batch, changes, vault, notes);
        let TagsBatch {
            mut touched,
            pages_changed,
            ..
        } = batch;
        if pages_changed {
            let graph = self.rebuild_graph();
            touched.extend(graph_keys(&self.graph));
            touched.extend(graph_keys(&graph));
            self.graph = graph;
        }
        touched.into_iter().collect()
    }
}

/// Los tags durante un lote, con lo que el lote va tocando.
struct TagsBatch<'t> {
    tags: &'t mut Tags,
    touched: BTreeSet<TagKey>,
    pages_changed: bool,
}

impl PerNoteIndex for TagsBatch<'_> {
    fn holds(&self, note: &VaultPath) -> bool {
        self.tags.index.note(note).is_some()
    }

    fn notes_within(&self, folder: &VaultPath) -> Vec<VaultPath> {
        self.tags
            .index
            .all()
            .map(|(note, _)| note)
            .filter(|note| note.starts_with(folder))
            .cloned()
            .collect()
    }

    fn forget(&mut self, note: &VaultPath) {
        self.pages_changed |= self.tags.drop_page(note);
        self.touched.extend(self.tags.index.remove(note));
    }

    /// Mueve las ocurrencias de `old` a `new`. Si cambia si es página, hay
    /// que releerla.
    fn moved(&mut self, old: &VaultPath, new: &VaultPath) -> Settle {
        let was_page = self.tags.drop_page(old);
        self.pages_changed |= was_page;
        let Some((note, keys)) = self.tags.index.take(old) else {
            return Settle::IfMissing;
        };
        self.touched.extend(keys);
        if was_page || page_name(&self.tags.folder, new).is_some() {
            return Settle::Rescan;
        }
        self.touched
            .extend(self.tags.index.insert(new.clone(), note));
        Settle::IfMissing
    }

    fn rescan(&mut self, note: &VaultPath, notes: &mut NoteBatch<'_>) {
        let (scanned, page) = scan_in_batch(&self.tags.folder, note, notes);
        let (touched, graph_stale) = self.tags.store(note.clone(), scanned, page);
        self.touched.extend(touched);
        self.pages_changed |= graph_stale;
    }
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
    use crate::note_scan::index_notes;
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

    /// Aplica cada lote en disco, en los tags y en las propiedades, y compara con
    /// armarlos de cero.
    fn check(batches: &[Vec<Op>]) -> Result<(), TestCaseError> {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let folder = VaultPath::fixed(DEFAULT_TAGS_FOLDER);
        let read = |path: &VaultPath| fs::read_to_string(path.to_fs(&root)).ok();
        let mut index = scan(&root, None);
        let mut tags = Tags::build(folder.clone(), &index, read);
        let opened = index_notes(&root, &index, &folder, None, &read);
        let (mut properties, mut summaries) = (opened.properties, opened.summaries);
        for batch in batches {
            let mut touched = Touched::default();
            for op in batch {
                touched.merge(apply(&root, op));
            }
            let changes = refresh(&mut index, &root, &touched, None);
            tags.apply(&changes, &index, &mut NoteBatch::new(&read));
            let full = Tags::build(folder.clone(), &scan(&root, None), read);
            prop_assert_eq!(&tags, &full, "tras {:?}: {:?}", batch, changes);
            properties.apply(&changes, &index, &mut NoteBatch::new(&read));
            summaries.apply(&changes, &index, &mut NoteBatch::new(&read));
            let full = index_notes(&root, &scan(&root, None), &folder, None, &read);
            prop_assert_eq!(
                &properties,
                &full.properties,
                "tras {:?}: {:?}",
                batch,
                changes
            );
            prop_assert_eq!(
                &summaries,
                &full.summaries,
                "tras {:?}: {:?}",
                batch,
                changes
            );
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
