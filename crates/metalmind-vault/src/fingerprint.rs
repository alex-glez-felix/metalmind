//! Reconocer una nota por su contenido: así se sigue un renombre hecho con
//! Metalmind cerrado, en todo lo que se guarda por ruta (06 §1).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::file;
use crate::history::ContentHash;
use crate::path::VaultPath;

/// El contenido de una nota: su largo en bytes y su hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Fingerprint {
    pub len: usize,
    pub hash: ContentHash,
}

impl Fingerprint {
    pub fn of(text: &str) -> Self {
        Self {
            len: text.len(),
            hash: ContentHash::of(text),
        }
    }

    /// La de la nota `path` de `root`; `None` si no existe o no se lee.
    pub fn of_note(root: &Path, path: &VaultPath) -> Option<Self> {
        file::read(&path.to_fs(root))
            .ok()
            .flatten()
            .map(|text| Self::of(&text))
    }
}

/// Para cada ruta perdida, la única de `candidates` con el mismo contenido.
/// Solo cuando el emparejamiento no tiene dudas: una nota vacía, o un
/// contenido que tienen más de una perdida o más de una candidata, se queda
/// sin pareja. Mira el tamaño antes de leer y hashear.
pub(crate) fn pair_by_content<'a>(
    root: &Path,
    lost: impl IntoIterator<Item = (VaultPath, Fingerprint)>,
    candidates: impl IntoIterator<Item = &'a VaultPath>,
) -> HashMap<VaultPath, &'a VaultPath> {
    let mut wanted: HashMap<Fingerprint, Vec<VaultPath>> = HashMap::new();
    for (path, fingerprint) in lost {
        if fingerprint.len > 0 {
            wanted.entry(fingerprint).or_default().push(path);
        }
    }
    let sizes: HashSet<usize> = wanted.keys().map(|fingerprint| fingerprint.len).collect();
    if sizes.is_empty() {
        return HashMap::new();
    }
    let mut found: HashMap<Fingerprint, Vec<&'a VaultPath>> = HashMap::new();
    for candidate in candidates {
        let same_size = fs::metadata(candidate.to_fs(root))
            .ok()
            .and_then(|metadata| usize::try_from(metadata.len()).ok())
            .is_some_and(|len| sizes.contains(&len));
        if !same_size {
            continue;
        }
        if let Some(fingerprint) = Fingerprint::of_note(root, candidate)
            && wanted.contains_key(&fingerprint)
        {
            found.entry(fingerprint).or_default().push(candidate);
        }
    }
    // Con dos perdidas del mismo contenido, nada dice cuál fue a dónde.
    wanted
        .into_iter()
        .filter_map(|(fingerprint, lost)| {
            match (lost.as_slice(), found.remove(&fingerprint)?.as_slice()) {
                ([lost], [target]) => Some((lost.clone(), *target)),
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(text: &str) -> VaultPath {
        VaultPath::new(text).unwrap()
    }

    #[test]
    fn solo_empareja_lo_que_no_tiene_dudas() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (name, text) in [
            ("nueva.md", "única"),
            ("gemela-1.md", "doble"),
            ("gemela-2.md", "doble"),
            ("vacía.md", ""),
            ("otra.md", "otro largo"),
        ] {
            fs::write(root.join(name), text).unwrap();
        }
        let candidates: Vec<VaultPath> = ["nueva.md", "gemela-1.md", "gemela-2.md", "vacía.md"]
            .into_iter()
            .map(path)
            .collect();
        let lost = [
            (path("vieja.md"), Fingerprint::of("única")),
            (path("doble.md"), Fingerprint::of("doble")),
            (path("era-vacía.md"), Fingerprint::of("")),
            (path("borrada.md"), Fingerprint::of("otro largo")),
        ];
        let paired = pair_by_content(root, lost, &candidates);
        assert_eq!(
            paired,
            HashMap::from([(path("vieja.md"), &candidates[0])]),
            "dos candidatas iguales, una vacía y una que no es candidata quedan sin pareja"
        );
    }

    #[test]
    fn dos_perdidas_del_mismo_contenido_no_se_emparejan() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("x.md"), "igual").unwrap();
        let candidates = [path("x.md")];
        let lost = [
            (path("a.md"), Fingerprint::of("igual")),
            (path("b.md"), Fingerprint::of("igual")),
        ];
        assert!(pair_by_content(dir.path(), lost, &candidates).is_empty());
    }
}
