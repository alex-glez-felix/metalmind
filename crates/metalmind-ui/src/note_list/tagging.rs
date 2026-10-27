//! `t` en la lista: agregar tags al frontmatter de una nota como una edición
//! del vault, que se aplica y se deshace como los cambios de tags (05 §5).

use metalmind_template::{TagsNotAdded, with_tags};
use metalmind_vault::{FileEdits, TextEdit, VaultPath};

/// La edición que deja `tags` en el `tags:` de la nota `path`, que dice
/// `text`, tocando solo ese campo; `None` si ya los tiene todos.
///
/// # Errors
/// Si un tag no es válido o el frontmatter no se puede editar sin
/// reescribirlo.
pub fn tag_edit(
    path: &VaultPath,
    text: &str,
    tags: &[String],
) -> Result<Option<FileEdits>, TagsNotAdded> {
    let tagged = with_tags(text, tags)?;
    if tagged == text {
        return Ok(None);
    }
    let start = common_prefix(text, &tagged);
    let end = common_suffix(&text[start..], &tagged[start..]);
    let edit = TextEdit {
        range: start..text.len() - end,
        old: text[start..text.len() - end].to_owned(),
        new: tagged[start..tagged.len() - end].to_owned(),
        line: text[..start].matches('\n').count(),
    };
    Ok(Some(FileEdits {
        path: path.clone(),
        edits: vec![edit],
        // Una nota vacía no tiene texto que comprobar: se exige su largo.
        base_len: Some(text.len()),
    }))
}

/// Bytes iguales al principio, sin partir un carácter.
fn common_prefix(a: &str, b: &str) -> usize {
    a.char_indices()
        .zip(b.chars())
        .find(|((_, x), y)| x != y)
        .map_or(a.len().min(b.len()), |((at, _), _)| at)
}

/// Bytes iguales al final, sin partir un carácter.
fn common_suffix(a: &str, b: &str) -> usize {
    a.chars()
        .rev()
        .zip(b.chars().rev())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| x.len_utf8())
        .sum()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn tagged(text: &str, tags: &[&str]) -> Option<String> {
        let tags: Vec<String> = tags.iter().map(|&tag| tag.to_owned()).collect();
        let path = VaultPath::new("nota.md").unwrap();
        let edits = tag_edit(&path, text, &tags).unwrap()?;
        let (after, _undo) = edits.apply(text).unwrap();
        Some(after)
    }

    #[rstest]
    #[case::sin_frontmatter("Una idea.\n", &["#rust"], "---\ntags: [rust]\n---\nUna idea.\n")]
    #[case::lista_de_flujo(
        "---\ntitle: x  # mío\ntags: [ñandú]\n---\nTexto\n",
        &["gpui", "ñandú"],
        "---\ntitle: x  # mío\ntags: [ñandú, gpui]\n---\nTexto\n"
    )]
    #[case::lista_en_bloque(
        "---\ntags:\n  - a\notra: 1\n---\n",
        &["b"],
        "---\ntags:\n  - a\n  - b\notra: 1\n---\n"
    )]
    fn solo_toca_el_campo_de_tags(
        #[case] text: &str,
        #[case] tags: &[&str],
        #[case] expected: &str,
    ) {
        assert_eq!(tagged(text, tags).as_deref(), Some(expected));
    }

    #[test]
    fn si_ya_los_tiene_no_hay_nada_que_editar() {
        assert_eq!(tagged("---\ntags: [Rust]\n---\n", &["rust"]), None);
    }

    #[test]
    fn una_nota_vacia_que_cambio_no_se_pisa() {
        let path = VaultPath::new("nota.md").unwrap();
        let edits = tag_edit(&path, "", &["rust".to_owned()]).unwrap().unwrap();
        assert!(edits.apply("Escrita mientras tanto.").is_err());
        assert!(edits.apply("").is_ok());
    }

    #[test]
    fn un_tag_invalido_no_edita() {
        let path = VaultPath::new("nota.md").unwrap();
        let error = tag_edit(&path, "x", &["a/b".to_owned()]).unwrap_err();
        assert_eq!(error, TagsNotAdded::InvalidTag("a/b".to_owned()));
    }

    #[test]
    fn la_edicion_es_el_tramo_que_cambia() {
        let path = VaultPath::new("nota.md").unwrap();
        let text = "---\ntags: [ñ]\n---\n";
        let edits = tag_edit(&path, text, &["é".to_owned()]).unwrap().unwrap();
        let edit = &edits.edits[0];
        assert_eq!(
            (edit.old.as_str(), edit.new.as_str(), edit.line),
            ("", ", é", 1)
        );
    }
}
