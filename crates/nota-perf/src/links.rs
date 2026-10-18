//! Un vault sintético con links, en memoria, para medir el índice de links.

use crate::text::WORD;

/// Cómo se llaman las notas de [`linked_vault`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteNames {
    /// Cada nombre se repite en las 100 carpetas, como en
    /// [`crate::vault_paths`]: cada link por nombre empata entre 100 rutas
    /// y lo desempata la carpeta propia. El peor caso.
    Repeated,
    /// Un nombre distinto por nota: lo común en un vault real.
    Unique,
}

/// Las notas de un vault de `notes` notas en 10 × 10 carpetas, más un
/// adjunto por carpeta, con su contenido (vacío en los adjuntos). Cada nota
/// tiene seis links: por nombre, por ruta, a un heading, markdown, un embed
/// y uno sin resolver.
#[must_use]
pub fn linked_vault(notes: usize, names: NoteNames) -> Vec<(String, String)> {
    let per_folder = notes.div_ceil(100);
    let name = |top: usize, sub: usize, note: usize| match names {
        NoteNames::Repeated => format!("{WORD}-{note}"),
        NoteNames::Unique => format!("{WORD}-{top}-{sub}-{note}"),
    };
    let mut files = Vec::with_capacity(notes + 100);
    for top in 0..10 {
        for sub in 0..10 {
            let dir = format!("area-{top}/tema-{sub}");
            for note in 0..per_folder {
                let next = name(top, sub, (note + 1) % per_folder);
                let beside = (top + 1) % 10;
                let elsewhere = name(beside, sub, note);
                let body = format!(
                    "# Nota {note}\n\nVer [[{next}]] y [[area-{beside}/tema-{sub}/{elsewhere}]].\n\
                     Más en [[{next}#Nota]] o [aquí]({next}.md).\n\
                     ![[imagen.png]] y [[pendiente-{note}]]\n"
                );
                files.push((format!("{dir}/{}.md", name(top, sub, note)), body));
            }
            files.push((format!("{dir}/imagen.png"), String::new()));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_paths_as_vault_paths_when_names_repeat() {
        let paths: Vec<String> = linked_vault(500, NoteNames::Repeated)
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        assert_eq!(paths, crate::vault_paths(500));
    }

    #[test]
    fn unique_names_do_not_repeat() {
        let files = linked_vault(500, NoteNames::Unique);
        let mut names: Vec<&str> = files
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .filter_map(|(path, _)| path.rsplit('/').next())
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 500);
    }
}
