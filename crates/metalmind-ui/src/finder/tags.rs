//! Los tags en `space f` (05 §4): `#rust -#archivado parser` deja las notas
//! con `#rust` (sus alias y descendientes) sin `#archivado`, y el fuzzy
//! busca `parser` entre ellas; `-#` deja el buzón.

use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use metalmind_vault::VaultPath;
use metalmind_vault::tags::{QueryWord, TagQuery, Tags};

use super::Note;
use crate::picker::Narrowing;

/// Las consultas de tags sobre los tags de un momento del vault.
pub struct TagFilter {
    tags: Arc<Tags>,
    /// La última consulta y sus notas: mientras solo cambia la parte difusa
    /// de lo escrito, no se vuelve a resolver.
    last: Mutex<Option<(TagQuery, Arc<HashSet<VaultPath>>)>>,
}

impl TagFilter {
    #[must_use]
    pub fn new(tags: Arc<Tags>) -> Self {
        Self {
            tags,
            last: Mutex::new(None),
        }
    }

    fn notes(&self, query: &TagQuery) -> Arc<HashSet<VaultPath>> {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((cached, notes)) = last.as_ref()
            && cached == query
        {
            return Arc::clone(notes);
        }
        let notes: Arc<HashSet<VaultPath>> =
            Arc::new(query.notes(&self.tags).into_iter().cloned().collect());
        *last = Some((query.clone(), Arc::clone(&notes)));
        notes
    }
}

impl Narrowing<Note> for TagFilter {
    fn fuzzy<'t>(&self, text: &'t str) -> Cow<'t, str> {
        let parsed = TagQuery::parse(text);
        if parsed.query.is_empty() && parsed.invalid.is_empty() {
            return Cow::Borrowed(text);
        }
        Cow::Owned(parsed.words.join(" "))
    }

    fn filters(&self, text: &str) -> bool {
        let parsed = TagQuery::parse(text);
        !parsed.query.is_empty() || !parsed.invalid.is_empty()
    }

    /// Un tag inválido (`#2024/10`) no deja ninguna.
    fn keep(&self, text: &str, items: &[Note]) -> Vec<usize> {
        let parsed = TagQuery::parse(text);
        if !parsed.invalid.is_empty() {
            return Vec::new();
        }
        let notes = self.notes(&parsed.query);
        items
            .iter()
            .enumerate()
            .filter(|(_, note)| notes.contains(&note.0))
            .map(|(index, _)| index)
            .collect()
    }

    /// Un tag inválido no deja ninguna nota; uno que no existe, incluido no
    /// deja ninguna y excluido no quita ninguna.
    fn warnings(&self, text: &str) -> Vec<Range<usize>> {
        TagQuery::words(text)
            .filter(|(_, word)| match word {
                QueryWord::Invalid => true,
                QueryWord::Include(key) | QueryWord::Exclude(key) => !self.tags.exists(key),
                QueryWord::Text | QueryWord::Untagged => false,
            })
            .map(|(range, _)| range)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use metalmind_search::FuzzyMatcher;
    use metalmind_vault::Vault;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::picker::{LineEdit, PickerState};

    const FILES: &[(&str, &str)] = &[
        ("parser.md", "#Rust y el parser"),
        ("gpui.md", "---\ntags: [gpui]\n---\nUn parser de #archivado"),
        ("viejo.md", "#rust #archivado"),
        ("sin-tags.md", "Nada."),
        ("tags/gpui.md", "---\nparents: [rust]\n---\n"),
    ];

    fn picker() -> PickerState<Note> {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in FILES {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        let notes = crate::finder::notes(&vault.snapshot(), &crate::finder::Recents::default());
        PickerState::new(notes).with_narrowing(Arc::new(TagFilter::new(vault.tags())))
    }

    /// Aplica `edit` y corre las búsquedas que pida, como el executor de fondo.
    fn edit(picker: &mut PickerState<Note>, edit: LineEdit<'_>) {
        let mut job = picker.edit(edit).job;
        while let Some(pending) = job {
            job = picker.finish(pending.run(&mut FuzzyMatcher::paths())).job;
        }
    }

    fn rows(picker: &mut PickerState<Note>, text: &str) -> Vec<String> {
        edit(picker, LineEdit::DeleteToStart);
        edit(picker, LineEdit::Insert(text));
        (0..picker.len())
            .map(|row| picker.item(row).unwrap().0.to_string())
            .collect()
    }

    /// Un tag trae a sus descendientes por `parents`, `-#` es el buzón y lo
    /// demás es el fuzzy de siempre entre lo que queda (05 §4).
    #[rstest::rstest]
    #[case::un_tag_y_sus_descendientes("#rust", &["gpui.md", "parser.md", "viejo.md"])]
    #[case::sin_mayusculas("#RUST -#archivado", &["parser.md"])]
    #[case::con_fuzzy("#rust parser", &["parser.md"])]
    #[case::solo_excluir("-#rust", &["sin-tags.md"])]
    #[case::buzon("-#", &["sin-tags.md"])]
    #[case::buzon_con_fuzzy("-# sin", &["sin-tags.md"])]
    #[case::tag_que_no_existe("#nada", &[])]
    #[case::excluir_uno_que_no_existe("-#nada viejo", &["viejo.md"])]
    #[case::doble_numeral("##rust", &[])]
    #[case::tag_invalido("#2024/10 parser", &[])]
    #[case::sin_tags("viejo", &["viejo.md"])]
    fn filtra_por_tags_y_busca_lo_demas(#[case] text: &str, #[case] expected: &[&str]) {
        let mut picker = picker();
        assert_eq!(rows(&mut picker, text), expected);
    }

    #[test]
    fn las_paginas_de_tag_no_salen_al_filtrar_por_tags() {
        let mut picker = picker();
        assert!(!rows(&mut picker, "-#archivado").contains(&"tags/gpui.md".to_owned()));
    }

    #[test]
    fn marca_los_tags_que_no_encuentran_nada() {
        let mut picker = picker();
        edit(
            &mut picker,
            LineEdit::Insert("#rust -#nada parser #a/b -# #Gpui ##rust"),
        );
        let text = picker.line().text().to_owned();
        let marked: Vec<&str> = picker
            .warnings()
            .into_iter()
            .map(|range| &text[range])
            .collect();
        assert_eq!(marked, ["-#nada", "#a/b", "##rust"]);
    }

    #[test]
    fn la_parte_difusa_no_vuelve_a_resolver_la_consulta() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "#rust").unwrap();
        let (vault, _updates) = Vault::open(dir.path()).unwrap();
        let filter = TagFilter::new(vault.tags());
        let query = TagQuery::parse("#rust").query;
        let first = filter.notes(&query);
        assert!(Arc::ptr_eq(&first, &filter.notes(&query)));
        assert!(!Arc::ptr_eq(
            &first,
            &filter.notes(&TagQuery::parse("-#rust").query)
        ));
    }
}
