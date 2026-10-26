use metalmind_vault::tags::{PlanNotice, Tags};
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};
use pretty_assertions::assert_eq;

use super::*;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Un vault con estas notas y su texto.
fn tags(notes: &[(&str, &str)]) -> Tags {
    let index = VaultIndex::from_entries(
        notes
            .iter()
            .map(|(name, _)| (path(name), EntryKind::Note))
            .chain([(path("tags"), EntryKind::Folder)]),
    );
    let read = |note: &VaultPath| {
        notes
            .iter()
            .find(|(name, _)| *name == note.as_str())
            .map(|(_, text)| (*text).to_owned())
    };
    Tags::build(path("tags"), &index, read)
}

/// `a.md` con un `#rust` y `b.md` con dos.
fn rust_notes() -> Tags {
    tags(&[("a.md", "uno #rust\n"), ("b.md", "#rust\ndos #Rust\n")])
}

fn preview(request: &TagRequest, tags: &Tags) -> TagChangePreview {
    let plan = request.plan(tags).unwrap();
    TagChangePreview::new(request.title(), plan, |_| false)
}

fn rename(from: &str, to: &str) -> TagRequest {
    TagRequest::Rename {
        from: from.to_owned(),
        to: to.to_owned(),
    }
}

#[test]
fn se_piden_dos_tags_con_o_sin_numeral() {
    assert_eq!(
        TagRequest::rename(Some("#rust  Rust")),
        Ok(rename("rust", "Rust"))
    );
    assert_eq!(
        TagRequest::merge(Some("rs #rust")),
        Ok(TagRequest::Merge {
            from: "rs".to_owned(),
            into: "rust".to_owned(),
        })
    );
    let error = TagRequest::rename(Some("#rust")).unwrap_err();
    assert_eq!(error.to_string(), "uso: :tagrename #viejo #nuevo");
    let error = TagRequest::merge(None).unwrap_err();
    assert_eq!(error.to_string(), "uso: :tagmerge #origen #destino");
    assert!(TagRequest::rename(Some("a b c")).is_err());
}

/// Una fila por ocurrencia, con su archivo, su línea desde 1 y el tag antes
/// y después.
#[test]
fn cada_ocurrencia_es_una_fila() {
    let preview = preview(&rename("rust", "lang"), &rust_notes());
    assert_eq!(preview.title(), "Renombrar #rust a #lang");
    let rows: Vec<(String, Option<usize>, String, String)> = (0..preview.len())
        .filter_map(|row| preview.row(row))
        .map(|row| {
            let (before, after) = (row.before.to_owned(), row.after.to_owned());
            (row.path.as_str().to_owned(), row.line, before, after)
        })
        .collect();
    let row = |file: &str, line, before: &str| {
        let (file, before) = (file.to_owned(), before.to_owned());
        (file, Some(line), before, "lang".to_owned())
    };
    assert_eq!(
        rows,
        [
            row("a.md", 1, "rust"),
            row("b.md", 1, "rust"),
            row("b.md", 2, "Rust")
        ]
    );
    assert_eq!(preview.summary(), "3 cambios en 2 archivos");
    assert_eq!(preview.blocked(), None);
}

/// `x` quita una fila de lo que se aplica y la vuelve a poner.
#[test]
fn quitar_una_fila_la_saca_de_lo_que_se_aplica() {
    let mut preview = preview(&rename("rust", "lang"), &rust_notes());
    let all = preview.changes();
    preview.toggle_selected();
    assert!(preview.row(0).unwrap().dropped);
    assert_eq!(preview.summary(), "2 cambios en 1 archivo");
    let changes = preview.changes();
    let files: Vec<&str> = changes
        .edits
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(files, ["b.md"]);
    assert_eq!(changes.edit_count(), 2);
    preview.toggle_selected();
    assert_eq!(preview.changes(), all);
}

#[test]
fn sin_cambios_no_se_aplica() {
    let mut preview = preview(&rename("rust", "lang"), &tags(&[("a.md", "#rust\n")]));
    preview.toggle_selected();
    assert_eq!(preview.summary(), "0 cambios en 0 archivos");
    assert_eq!(preview.blocked(), Some(Blocked::Nothing));
}

/// Un archivo con cambios sin guardar bloquea mientras tenga alguna fila
/// que se aplica.
#[test]
fn un_archivo_sin_guardar_bloquea_mientras_se_toque() {
    let request = rename("rust", "lang");
    let plan = request.plan(&rust_notes()).unwrap();
    let mut preview = TagChangePreview::new(request.title(), plan, |file| file.as_str() == "a.md");
    assert!(preview.row(0).unwrap().unsaved);
    assert_eq!(
        preview.blocked(),
        Some(Blocked::Unsaved(vec![&path("a.md")]))
    );
    preview.toggle_selected();
    assert_eq!(preview.blocked(), None);
}

/// Renombrar un tag con página la renombra en la última fila, sin línea.
#[test]
fn la_pagina_se_renombra_en_su_fila() {
    let tags = tags(&[("a.md", "#rust\n"), ("tags/rust.md", "Del lenguaje.\n")]);
    let preview = preview(&rename("rust", "lang"), &tags);
    let last = preview.row(preview.len() - 1).unwrap();
    assert_eq!(
        (last.path.as_str(), last.line, last.before, last.after),
        ("tags/rust.md", None, "tags/rust.md", "tags/lang.md")
    );
    assert_eq!(preview.changes().renames.len(), 1);
}

/// Si los dos tags tienen página, el plan lo marca y no se aplica.
#[test]
fn dos_paginas_bloquean_la_fusion() {
    let tags = tags(&[
        ("a.md", "#rust #go\n"),
        ("tags/rust.md", "R\n"),
        ("tags/go.md", "G\n"),
    ]);
    let request = TagRequest::Merge {
        from: "rust".to_owned(),
        into: "go".to_owned(),
    };
    let preview = preview(&request, &tags);
    assert_eq!(preview.title(), "Fusionar #rust en #go");
    assert!(matches!(
        preview.blocked(),
        Some(Blocked::Notice(PlanNotice::BothHavePages { .. }))
    ));
}

#[test]
fn elegir_da_la_vuelta() {
    let mut preview = preview(&rename("rust", "lang"), &rust_notes());
    preview.select_previous();
    assert_eq!(preview.selected(), 2);
    preview.select_next();
    assert_eq!(preview.selected(), 0);
    preview.select(9);
    assert_eq!(preview.selected(), 0);
    preview.select(1);
    assert_eq!(preview.selected(), 1);
}
