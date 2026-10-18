//! Las notas abiertas hace poco en un vault, de la última a la primera. Se
//! guardan en su directorio de estado (06 §8) como texto, una ruta del
//! vault por línea, y siguen los renombres que trae el watcher (06 §1).

use nota_vault::{VaultChange, VaultPath};

/// Cuántas se recuerdan.
const LIMIT: usize = 100;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recents {
    paths: Vec<VaultPath>,
}

impl Recents {
    /// Lee lo guardado; una línea que no es una ruta del vault se ignora.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut recents = Self::default();
        for line in text.lines() {
            if let Ok(path) = VaultPath::new(line)
                && !path.is_root()
                && !recents.paths.contains(&path)
                && recents.paths.len() < LIMIT
            {
                recents.paths.push(path);
            }
        }
        recents
    }

    /// Lo que se guarda: una ruta por línea, la más reciente primero.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for path in &self.paths {
            text.push_str(path.as_str());
            text.push('\n');
        }
        text
    }

    #[must_use]
    pub fn paths(&self) -> &[VaultPath] {
        &self.paths
    }

    /// `path` pasa a ser la más reciente. Devuelve si cambió algo. Una ruta
    /// con un salto de línea no cabe en el archivo y no se recuerda.
    pub fn touch(&mut self, path: VaultPath) -> bool {
        if self.paths.first() == Some(&path) || path.as_str().contains('\n') {
            return false;
        }
        self.paths.retain(|recent| *recent != path);
        self.paths.insert(0, path);
        self.paths.truncate(LIMIT);
        true
    }

    /// Sigue un cambio en disco: un renombre mueve la entrada (también lo
    /// que había dentro de una carpeta) y un borrado la quita.
    pub fn apply(&mut self, change: &VaultChange) -> bool {
        let before = self.paths.clone();
        match change {
            VaultChange::Renamed { from, to, .. } => {
                for path in &mut self.paths {
                    if let Some(moved) = path.rebase(from, to) {
                        *path = moved;
                    }
                }
            }
            VaultChange::Removed { path: removed, .. } => {
                self.paths.retain(|path| !path.starts_with(removed));
            }
            VaultChange::Created { .. } | VaultChange::Modified { .. } => {}
        }
        self.paths != before
    }
}

#[cfg(test)]
mod tests {
    use nota_vault::EntryKind;
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(text: &str) -> VaultPath {
        VaultPath::new(text).unwrap()
    }

    fn recents(paths: &[&str]) -> Recents {
        Recents {
            paths: paths.iter().copied().map(path).collect(),
        }
    }

    #[test]
    fn la_ultima_abierta_va_primero_sin_repetirse() {
        let mut recents = Recents::default();
        assert!(recents.touch(path("a.md")));
        assert!(recents.touch(path("b.md")));
        assert!(recents.touch(path("a.md")));
        assert!(!recents.touch(path("a.md")));
        assert_eq!(recents, self::recents(&["a.md", "b.md"]));
    }

    #[test]
    fn recuerda_un_maximo() {
        let mut recents = Recents::default();
        for n in 0..LIMIT + 5 {
            recents.touch(path(&format!("{n}.md")));
        }
        assert_eq!(recents.paths().len(), LIMIT);
        assert_eq!(recents.paths()[0], path(&format!("{}.md", LIMIT + 4)));
    }

    #[test]
    fn ida_y_vuelta_por_el_archivo() {
        let recents = recents(&["notas/b c.md", "a.md"]);
        assert_eq!(recents.to_text(), "notas/b c.md\na.md\n");
        assert_eq!(Recents::parse(&recents.to_text()), recents);
    }

    #[test]
    fn lo_que_no_es_una_ruta_del_vault_se_ignora() {
        let parsed = Recents::parse("a.md\n\n/abs.md\n../fuera.md\n.oculta/x.md\na.md\nb.md\n");
        assert_eq!(parsed, recents(&["a.md", "b.md"]));
    }

    #[test]
    fn sigue_los_renombres_tambien_de_carpetas() {
        let mut recents = recents(&["viejo/a.md", "b.md", "viejo/sub/c.md", "viejos.md"]);
        let renamed = VaultChange::Renamed {
            from: path("viejo"),
            to: path("nuevo"),
            kind: EntryKind::Folder,
        };
        assert!(recents.apply(&renamed));
        assert_eq!(
            recents,
            self::recents(&["nuevo/a.md", "b.md", "nuevo/sub/c.md", "viejos.md"])
        );
        let note = VaultChange::Renamed {
            from: path("b.md"),
            to: path("x/b.md"),
            kind: EntryKind::Note,
        };
        assert!(recents.apply(&note));
        assert_eq!(recents.paths()[1], path("x/b.md"));
    }

    #[test]
    fn un_borrado_las_quita() {
        let mut recents = recents(&["d/a.md", "b.md", "d/c.md"]);
        let removed = VaultChange::Removed {
            path: path("d"),
            kind: EntryKind::Folder,
        };
        assert!(recents.apply(&removed));
        assert_eq!(recents, self::recents(&["b.md"]));
        let modified = VaultChange::Modified { path: path("b.md") };
        assert!(!recents.apply(&modified));
    }
}
