use metalmind_vault::tags::{PageChange, Tags};
use metalmind_vault::{EntryKind, VaultIndex, VaultPath};
use pretty_assertions::assert_eq;

use super::*;
use crate::change_preview::Blocked;

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

fn preview(request: &TagRequest, tags: &Tags) -> ChangePreview {
    request
        .preview(tags, &VaultConfigText::Missing, |_| false)
        .unwrap()
        .unwrap()
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
/// Armada de nuevo con otros textos (una nota que se guardó con la vista
/// previa abierta), sigue quitado lo que se había quitado y elegida la
/// misma fila, aunque cambie de línea.
#[test]
fn armarla_de_nuevo_deja_lo_quitado() {
    let request = rename("rust", "lang");
    let mut old = preview(&request, &rust_notes());
    old.select(2);
    old.toggle_selected();
    let quitada = old.row(2).unwrap();
    let (path, before) = (quitada.path.clone(), quitada.before.to_owned());
    let saved = tags(&[
        ("a.md", "nueva línea\nuno #rust\n"),
        ("b.md", "#rust\ndos #Rust\n"),
    ]);
    let mut new = preview(&request, &saved);
    new.carry_over(&old);
    let dropped: Vec<(String, String)> = (0..new.len())
        .filter_map(|row| new.row(row))
        .filter(|row| row.dropped)
        .map(|row| (row.path.as_str().to_owned(), row.before.to_owned()))
        .collect();
    assert_eq!(dropped, [(path.as_str().to_owned(), before)]);
    assert_eq!(new.selected(), 2);
    assert_eq!(new.row(0).unwrap().line, Some(2), "a.md cambió de línea");
}

/// Si lo editado entre medio agrega filas antes de la elegida, sigue
/// elegida la misma fila, no la que quedó en su número; si ya no está, la
/// de su lugar.
#[test]
fn armarla_de_nuevo_elige_la_misma_fila_aunque_se_corra() {
    let request = rename("rust", "lang");
    let mut old = preview(&request, &rust_notes());
    old.select(1);
    let elegida = old.row(1).unwrap();
    let (path, before) = (elegida.path.clone(), elegida.before.to_owned());
    let more = tags(&[
        ("a.md", "uno #rust\notro #rust\n"),
        ("b.md", "#rust\ndos #Rust\n"),
    ]);
    let mut new = preview(&request, &more);
    new.carry_over(&old);
    let row = new.row(new.selected()).unwrap();
    assert_eq!((row.path, row.before), (&path, before.as_str()));
    assert_eq!(new.selected(), 2, "a.md tiene una fila más");

    let gone = tags(&[("a.md", "uno #rust\n")]);
    let mut fewer = preview(&request, &gone);
    fewer.carry_over(&old);
    assert_eq!(fewer.selected(), 0, "b.md ya no está: la de su lugar");
}

/// Dos líneas iguales dan dos filas iguales: quitar la segunda no quita la
/// primera al rehacerla, y la elegida sigue siendo la segunda.
#[test]
fn dos_filas_iguales_se_distinguen_al_rehacerla() {
    let request = rename("rust", "lang");
    let same = tags(&[("a.md", "uno #rust\nuno #rust\n")]);
    let mut old = preview(&request, &same);
    old.select(1);
    old.toggle_selected();
    let mut new = preview(&request, &same);
    new.carry_over(&old);
    let dropped: Vec<bool> = (0..new.len())
        .map(|row| new.row(row).unwrap().dropped)
        .collect();
    assert_eq!(dropped, [false, true]);
    assert_eq!(new.selected(), 1);
}

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

/// Las ediciones de un archivo con cambios sin guardar van a su buffer: se
/// marcan y no bloquean (05 §5).
#[test]
fn una_edicion_en_un_buffer_sin_guardar_no_bloquea() {
    let request = rename("rust", "lang");
    let preview = request
        .preview(&rust_notes(), &VaultConfigText::Missing, |file| {
            file.as_str() == "a.md"
        })
        .unwrap()
        .unwrap();
    let row = preview.row(0).unwrap();
    assert_eq!(
        (row.path, row.unsaved, row.in_buffer),
        (&path("a.md"), None, true)
    );
    assert!(!preview.row(1).unwrap().in_buffer, "b.md va al disco");
    assert_eq!(preview.blocked(), None);
}

/// La página con cambios sin guardar no se renombra por debajo de su
/// buffer: bloquea mientras no se guarde, como al autoguardar.
#[test]
fn renombrar_una_pagina_sin_guardar_bloquea_hasta_guardarla() {
    let tags = tags(&[("a.md", "#rust\n"), ("tags/rust.md", "Del lenguaje.\n")]);
    let request = rename("rust", "lang");
    let mut preview = request
        .preview(&tags, &VaultConfigText::Missing, |file| {
            file.as_str() == "tags/rust.md"
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        preview.blocked(),
        Some(Blocked::Unsaved(vec![&path("tags/rust.md")]))
    );
    preview.set_unsaved(|_| false);
    assert_eq!(preview.blocked(), None);
}

/// Un buffer sin guardar en el destino de un renombre de página también
/// bloquea: quedarían dos documentos en la misma ruta.
#[test]
fn un_buffer_sin_guardar_en_el_destino_bloquea() {
    let tags = tags(&[("a.md", "#rust\n"), ("tags/rust.md", "Del lenguaje.\n")]);
    let request = rename("rust", "lang");
    let preview = request
        .preview(&tags, &VaultConfigText::Missing, |file| {
            file.as_str() == "tags/lang.md"
        })
        .unwrap()
        .unwrap();
    let last = preview.row(preview.len() - 1).unwrap();
    assert_eq!(last.unsaved, Some(&path("tags/lang.md")));
    assert_eq!(
        preview.blocked(),
        Some(Blocked::Unsaved(vec![&path("tags/lang.md")]))
    );
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

/// Fusionar dos tags que solo tienen página no tiene ediciones, pero se
/// muestra igual: dice por qué no se puede. Sin cambios ni avisos, no hay
/// vista previa.
#[test]
fn un_bloqueo_sin_cambios_se_muestra_y_nada_no() {
    let pages = tags(&[("tags/a.md", "A\n"), ("tags/b.md", "B\n")]);
    let request = TagRequest::Merge {
        from: "a".to_owned(),
        into: "b".to_owned(),
    };
    let preview = request
        .preview(&pages, &VaultConfigText::Missing, |_| false)
        .unwrap()
        .unwrap();
    assert!(matches!(
        preview.blocked(),
        Some(Blocked::Notice(Notice { blocks: true, text })) if text.contains("tienen página")
    ));
    let nothing = TagRequest::Rename {
        from: "a".to_owned(),
        to: "a".to_owned(),
    };
    assert!(
        nothing
            .preview(&pages, &VaultConfigText::Missing, |_| false)
            .unwrap()
            .is_none()
    );
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
        Some(Blocked::Notice(Notice { blocks: true, text })) if text.contains("tienen página")
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

fn page(field: TagField, argument: &str) -> Result<TagRequest, RequestError> {
    TagRequest::page(field, Some(argument))
}

/// `:tagaliases #rust rs, rustlang` y los demás: el tag y el valor nuevo,
/// que vacío quita el campo.
#[test]
fn los_campos_de_la_pagina_se_piden_con_el_tag_y_su_valor() {
    let aliases = page(TagField::Aliases, "#rust rs, rustlang").unwrap();
    assert_eq!(
        aliases,
        TagRequest::Page {
            tag: "rust".to_owned(),
            field: TagField::Aliases,
            change: PageChange::Aliases(vec!["rs".to_owned(), "rustlang".to_owned()]),
        }
    );
    assert_eq!(aliases.title(), "Alias de #rust");
    let no_icon = page(TagField::Icon, "rust").unwrap();
    assert!(matches!(
        no_icon,
        TagRequest::Page {
            change: PageChange::Icon(None),
            ..
        }
    ));
    assert_eq!(
        page(TagField::Template, "#reunion").unwrap().title(),
        "Plantilla de #reunion"
    );
    let usage = TagRequest::page(TagField::Parents, None).unwrap_err();
    assert_eq!(usage.to_string(), "uso: :tagparents #tag [padre…]");
    let icon = page(TagField::Icon, "#rust libro").unwrap_err();
    assert_eq!(
        icon.to_string(),
        "`libro` no es un ícono: va un emoji o `set:nombre`"
    );
}

/// Sin página, el plan la crea con el campo; con página, edita solo el
/// campo.
#[test]
fn el_plan_crea_la_pagina_o_edita_su_campo() {
    let request = page(TagField::Icon, "#rust 🦀").unwrap();
    let plan = request.plan(&rust_notes()).unwrap();
    let created: Vec<(&str, &str)> = plan
        .changes
        .creates
        .iter()
        .map(|file| (file.path.as_str(), file.text.as_str()))
        .collect();
    assert_eq!(created, [("tags/rust.md", "---\nicon: 🦀\n---\n")]);

    let with_page = tags(&[
        ("a.md", "#rust\n"),
        (
            "tags/rust.md",
            "---\nicon: 🦀\naliases: [rs]\n---\nEl lenguaje.\n",
        ),
    ]);
    let request = page(TagField::Template, "#rust rust/capitulo").unwrap();
    let plan = request.plan(&with_page).unwrap();
    assert_eq!(plan.changes.creates, []);
    assert_eq!(plan.changes.edits.len(), 1);
    assert_eq!(plan.changes.edits[0].path.as_str(), "tags/rust.md");
}

/// El valor de ahora, como lo pide la pregunta del panel.
#[test]
fn lo_que_tiene_cada_campo() {
    let with_page = tags(&[
        ("a.md", "#rust\n"),
        (
            "tags/rust.md",
            "---\naliases: [rs, Rustlang]\nicon: lucide:book\ntemplate: rust/capitulo\n---\n",
        ),
    ]);
    let current = |field: TagField, tag: &str| field.current(&with_page, tag);
    assert_eq!(current(TagField::Aliases, "rust"), "rs, Rustlang");
    assert_eq!(current(TagField::Icon, "#rust"), "lucide:book");
    assert_eq!(
        current(TagField::Template, "rs"),
        "rust/capitulo",
        "un alias es su tag"
    );
    assert_eq!(current(TagField::Parents, "rust"), "");
    assert_eq!(current(TagField::Aliases, "otro"), "");
}

/// La fila de un campo de la página muestra solo las líneas que cambian:
/// el campo nuevo sin el `---` en que se ancla, el frontmatter nuevo sin la
/// primera línea de la descripción.
#[test]
fn la_fila_muestra_solo_las_lineas_que_cambian() {
    let pages = tags(&[
        ("a.md", "#rust #go\n"),
        ("tags/rust.md", "---\naliases: [rs]\n---\nEl lenguaje.\n"),
        ("tags/go.md", "Otro lenguaje.\nSegunda línea.\n"),
    ]);
    let shown = |request: &str| {
        let (field, argument) = request.split_once(' ').unwrap();
        let field = match field {
            "aliases" => TagField::Aliases,
            "icon" => TagField::Icon,
            _ => TagField::Template,
        };
        let preview = preview(&page(field, argument).unwrap(), &pages);
        let row = preview.row(0).unwrap();
        (row.line, row.before.to_owned(), row.after.to_owned())
    };
    assert_eq!(
        shown("aliases #rust rs, rustlang"),
        (
            Some(2),
            "aliases: [rs]".to_owned(),
            "aliases: [rs, rustlang]".to_owned()
        )
    );
    assert_eq!(
        shown("template #rust rust/capitulo"),
        (Some(3), String::new(), "template: rust/capitulo".to_owned())
    );
    assert_eq!(
        shown("icon #go lucide:book"),
        (
            Some(1),
            String::new(),
            "---\nicon: lucide:book\n---".to_owned()
        )
    );
}

const VIEWS: &str =
    "# vistas\nviews:\n  rust: \"#rust -#archivado\"  # la de siempre\n  hoy: \"#hoy\"\n";

fn rows(preview: &ChangePreview) -> Vec<(String, Option<usize>, String, String)> {
    (0..preview.len())
        .filter_map(|row| preview.row(row))
        .map(|row| {
            let (before, after) = (row.before.to_owned(), row.after.to_owned());
            (row.path.as_str().to_owned(), row.line, before, after)
        })
        .collect()
}

/// Una vista guardada que nombra el tag sale como una fila más, en la
/// config del vault; las demás vistas no cambian.
#[test]
fn las_vistas_que_nombran_el_tag_lo_siguen() {
    let config = VaultConfigText::Text(VIEWS.to_owned());
    let preview = rename("rust", "lang")
        .preview(&rust_notes(), &config, |_| false)
        .unwrap()
        .unwrap();
    assert_eq!(
        rows(&preview)[0],
        (
            ".metalmind/config.yaml".to_owned(),
            Some(3),
            "\"#rust -#archivado\"".to_owned(),
            "\"#lang -#archivado\"".to_owned()
        )
    );
    assert_eq!(preview.summary(), "4 cambios en 3 archivos");
    let merge = TagRequest::Merge {
        from: "hoy".to_owned(),
        into: "rust".to_owned(),
    };
    let tags = tags(&[("a.md", "#rust #hoy\n")]);
    let preview = merge.preview(&tags, &config, |_| false).unwrap().unwrap();
    let (_, _, before, after) = rows(&preview).swap_remove(0);
    assert_eq!((before.as_str(), after.as_str()), ("\"#hoy\"", "\"#rust\""));
}

/// Una config que no se puede reescribir no impide renombrar el tag:
/// la vista previa dice que sus vistas no lo siguen.
#[test]
fn una_config_rota_avisa_sin_bloquear() {
    for config in [
        VaultConfigText::Text("views: [\n".to_owned()),
        VaultConfigText::Text("views: {rust: \"#rust\"}\n".to_owned()),
        VaultConfigText::Unreadable("permiso denegado".to_owned()),
    ] {
        let preview = rename("rust", "lang")
            .preview(&rust_notes(), &config, |_| false)
            .unwrap()
            .unwrap();
        assert_eq!(preview.len(), 3, "{config:?}");
        let notice = &preview.notices()[0];
        assert!(!notice.blocks);
        assert!(
            notice
                .text
                .starts_with("las vistas guardadas no siguen a #lang: "),
            "{}",
            notice.text
        );
    }
}
