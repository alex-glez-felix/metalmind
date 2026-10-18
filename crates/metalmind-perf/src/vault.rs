//! Un vault sintético en disco, con las rutas de [`vault_paths`] y notas
//! chicas y distintas entre sí, para medir lo que lee todas las notas.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use crate::text::{Lcg, paragraph_line, vault_paths};

/// Palabra que sale en una de cada [`RARE_EVERY`] notas de [`write_vault`]:
/// buscarla cuesta lo que cuesta leer todo el vault.
pub const RARE_WORD: &str = "zafiro";

/// Cada cuántas notas sale [`RARE_WORD`].
pub const RARE_EVERY: usize = 1_000;

const LINES_PER_NOTE: usize = 24;

/// La nota `number` del vault: frontmatter con tags y fecha, un encabezado y
/// párrafos de ~1.5 KB. Cada número da otro texto, siempre el mismo.
#[must_use]
pub fn vault_note(number: usize) -> String {
    let mut rng = Lcg(0x7661_756c_7400 ^ number as u64);
    let mut note = String::with_capacity(LINES_PER_NOTE * 72);
    let _ = write!(
        note,
        "---\ntags: [tema-{}]\nfecha: 2026-{:02}-{:02}\n---\n# Nota {number}\n\n",
        number % 37,
        1 + number % 12,
        1 + number % 28,
    );
    let mut line = String::new();
    for index in 0..LINES_PER_NOTE {
        line.clear();
        paragraph_line(&mut line, &mut rng, index);
        note.push_str(&line);
        note.push('\n');
    }
    if number.is_multiple_of(RARE_EVERY) {
        let _ = writeln!(note, "un {RARE_WORD} escondido");
    }
    note
}

/// Escribe en `root` un vault de `notes` notas ([`vault_note`]) en las rutas
/// de [`vault_paths`], con su adjunto por carpeta.
///
/// # Errors
/// Si falla la escritura.
pub fn write_vault(root: &Path, notes: usize) -> io::Result<()> {
    let mut number = 0;
    for path in vault_paths(notes) {
        let path = root.join(path);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        if path.extension().is_some_and(|ext| ext == "md") {
            fs::write(&path, vault_note(number))?;
            number += 1;
        } else {
            fs::write(&path, [0_u8; 64])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_notas_son_distintas_y_deterministas() {
        assert_eq!(vault_note(7), vault_note(7));
        assert_ne!(vault_note(7), vault_note(8));
        let len = vault_note(7).len();
        assert!((1_000..2_500).contains(&len), "{len}");
    }

    #[test]
    fn la_palabra_rara_sale_una_vez_cada_mil() {
        let with_rare = (0..5_000)
            .filter(|&number| vault_note(number).contains(RARE_WORD))
            .count();
        assert_eq!(with_rare, 5);
    }

    #[test]
    fn write_vault_escribe_las_rutas_de_vault_paths() {
        let dir = tempfile::TempDir::new().unwrap();
        write_vault(dir.path(), 500).unwrap();
        for path in vault_paths(500) {
            assert!(dir.path().join(path).is_file());
        }
    }
}
