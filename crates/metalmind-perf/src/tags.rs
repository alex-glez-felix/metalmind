//! Un vault sintético con tags y páginas de tag, en memoria, para medir el
//! índice de tags (05 §9).

use std::fs;
use std::io;
use std::path::Path;

use crate::text::vault_paths;

/// La página `tags/t<index>.md`. Forman un árbol: `t<i>` tiene de padre a
/// `t<i/10>`, y las múltiplo de 7 también a `t1`, para que haya varios
/// padres. Cada una tiene color y el alias `a<index>`.
fn tag_page(index: usize) -> String {
    let mut parents = Vec::new();
    if index > 0 {
        parents.push(format!("t{}", index / 10));
    }
    if index.is_multiple_of(7) && index > 1 {
        parents.push("t1".to_owned());
    }
    format!(
        "---\ncolor: naranja\naliases: [a{index}]\nparents: [{}]\n---\nDe qué trata #t{index}.\n",
        parents.join(", ")
    )
}

/// La nota `number`: una de cada diez sin tags; las demás con un tag en
/// `tags:`, dos en el texto (uno con otras mayúsculas) y un alias.
fn tagged_note(number: usize, pages: usize) -> String {
    if number.is_multiple_of(10) {
        return format!("# Nota {number}\n\nSin tags.\n");
    }
    format!(
        "---\ntags: [t{}]\n---\n# Nota {number}\n\nVer #t{} y #T{}, también #a{}.\n",
        number % pages,
        (number * 7) % pages,
        (number * 13) % pages,
        (number * 3) % pages,
    )
}

/// Las rutas de [`vault_paths`] con su contenido (vacío en los adjuntos),
/// más `pages` páginas de tag en `tags/`.
#[must_use]
pub fn tagged_vault(notes: usize, pages: usize) -> Vec<(String, String)> {
    let mut number = 0;
    let mut files: Vec<(String, String)> = vault_paths(notes)
        .into_iter()
        .map(|path| {
            let text = if Path::new(&path).extension().is_some_and(|ext| ext == "md") {
                number += 1;
                tagged_note(number - 1, pages)
            } else {
                String::new()
            };
            (path, text)
        })
        .collect();
    files.extend((0..pages).map(|index| (format!("tags/t{index}.md"), tag_page(index))));
    files
}

/// Escribe `files` bajo `root`, con sus carpetas.
///
/// # Errors
/// Si falla la escritura.
pub fn write_files(root: &Path, files: &[(String, String)]) -> io::Result<()> {
    for (path, text) in files {
        let path = root.join(path);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_paginas_y_las_notas_estan_todas() {
        let files = tagged_vault(500, 30);
        assert_eq!(files.len(), 500 + 100 + 30);
        let untagged = files
            .iter()
            .filter(|(_, text)| text.contains("Sin tags"))
            .count();
        assert_eq!(untagged, 50);
        assert!(
            files
                .iter()
                .any(|(path, text)| path == "tags/t21.md" && text.contains("parents: [t2, t1]"))
        );
    }
}
