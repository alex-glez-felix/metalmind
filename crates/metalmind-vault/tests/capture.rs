#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::fs;

use jiff::civil::{DateTime, date};
use metalmind_vault::{CaptureError, NewNote, VaultRoot};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn now() -> DateTime {
    date(2026, 10, 18).at(9, 5, 7, 0)
}

fn note(text: &str) -> NewNote {
    NewNote {
        text: text.to_owned(),
        ..NewNote::default()
    }
}

fn capture(dir: &TempDir, note: &NewNote) -> Result<(String, String), CaptureError> {
    let vault = VaultRoot::new(dir.path()).unwrap();
    let captured = vault.capture(note, now())?;
    let text = fs::read_to_string(dir.path().join(captured.path.as_str())).unwrap();
    Ok((captured.path.to_string(), text))
}

#[test]
fn sin_tags_cae_en_el_buzon() {
    let dir = TempDir::new().unwrap();
    let (path, text) = capture(&dir, &note("Comprar leche\ny pan\n")).unwrap();
    assert_eq!(path, "comprar-leche.md");
    assert_eq!(text, "Comprar leche\ny pan\n");
    let scan = metalmind_syntax::scan(&text);
    assert_eq!(scan.frontmatter, None);
    assert_eq!(scan.tags, []);
}

#[test]
fn los_tags_van_al_frontmatter_y_el_scanner_los_lee() {
    let dir = TempDir::new().unwrap();
    let new = NewNote {
        tags: ["#rust", "2024", "true", "canción", "rust"]
            .map(String::from)
            .to_vec(),
        ..note("idea")
    };
    let vault = VaultRoot::new(dir.path()).unwrap();
    let captured = vault.capture(&new, now()).unwrap();
    assert_eq!(captured.tags, ["rust", "2024", "true", "canción"]);
    let text = fs::read_to_string(dir.path().join(captured.path.as_str())).unwrap();
    let scan = metalmind_syntax::scan(&text);
    assert_eq!(scan.problems, []);
    let read: Vec<String> = scan
        .frontmatter
        .unwrap()
        .tags
        .into_iter()
        .map(|tag| tag.text)
        .collect();
    assert_eq!(read, captured.tags);
}

#[test]
fn el_titulo_da_el_nombre_y_el_encabezado() {
    let dir = TempDir::new().unwrap();
    let new = NewNote {
        title: Some("Reunión del lunes".to_owned()),
        ..note("- acordar fecha")
    };
    let (path, text) = capture(&dir, &new).unwrap();
    assert_eq!(path, "reunión-del-lunes.md");
    assert_eq!(text, "# Reunión del lunes\n\n- acordar fecha\n");
}

#[test]
fn un_nombre_tomado_prueba_el_siguiente_sin_pisar() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("idea.md"), "vieja\n").unwrap();
    let (second, _) = capture(&dir, &note("idea")).unwrap();
    let (third, _) = capture(&dir, &note("Idea")).unwrap();
    assert_eq!(second, "idea-2.md");
    assert_eq!(third, "idea-3.md");
    let old = fs::read_to_string(dir.path().join("idea.md")).unwrap();
    assert_eq!(old, "vieja\n");
}

#[test]
fn sin_letras_el_nombre_es_la_fecha_y_la_hora() {
    let dir = TempDir::new().unwrap();
    let (path, text) = capture(&dir, &note("🔥🔥")).unwrap();
    assert_eq!(path, "2026-10-18-090507.md");
    assert_eq!(text, "🔥🔥\n");
}

#[test]
fn respeta_el_frontmatter_que_trae_el_texto() {
    let dir = TempDir::new().unwrap();
    let (path, text) = capture(&dir, &note("---\ntags: [a]\n---\nCuerpo\n")).unwrap();
    assert_eq!(path, "cuerpo.md");
    assert_eq!(text, "---\ntags: [a]\n---\nCuerpo\n");
    let with_tags = NewNote {
        tags: vec!["b".to_owned()],
        ..note("---\ntags: [a]\n---\nCuerpo\n")
    };
    assert!(matches!(
        capture(&dir, &with_tags),
        Err(CaptureError::FrontmatterWithTags)
    ));
}

#[test]
fn rechaza_lo_que_no_es_una_nota() {
    let dir = TempDir::new().unwrap();
    assert!(matches!(
        capture(&dir, &note(" \n\t")),
        Err(CaptureError::Empty)
    ));
    let multiline = NewNote {
        title: Some("a\nb".to_owned()),
        ..note("x")
    };
    assert!(matches!(
        capture(&dir, &multiline),
        Err(CaptureError::MultilineTitle)
    ));
    let bad_tag = NewNote {
        tags: vec!["con espacio".to_owned()],
        ..note("x")
    };
    assert!(matches!(
        capture(&dir, &bad_tag),
        Err(CaptureError::InvalidTag(_))
    ));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

/// `capture_with` arma el texto con lo que la captura decidió (tags sin `#`
/// ni repetidos, título y nombre), y nombra y escribe igual que `capture`.
#[test]
fn capture_with_deja_el_texto_a_quien_llama() {
    let dir = TempDir::new().unwrap();
    let vault = VaultRoot::new(dir.path()).unwrap();
    let new = NewNote {
        text: "lo hablado".to_owned(),
        title: Some(" Reunión ".to_owned()),
        tags: ["#trabajo", "Trabajo"].map(String::from).to_vec(),
    };
    let mut seen = None;
    let captured = vault
        .capture_with(&new, now(), |capture| {
            seen = Some(format!("{capture:?}"));
            format!("{} / {}\n", capture.stem, capture.tags.join(","))
        })
        .unwrap();
    assert_eq!(captured.path.as_str(), "reunión.md");
    assert_eq!(
        seen.unwrap(),
        r#"Capture { tags: ["trabajo"], title: Some("Reunión"), stem: "reunión", text: "lo hablado" }"#
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("reunión.md")).unwrap(),
        "reunión / trabajo\n"
    );
    let invalid = NewNote {
        tags: vec!["con espacio".to_owned()],
        ..new
    };
    assert!(matches!(
        vault.capture_with(&invalid, now(), |_| String::new()),
        Err(CaptureError::InvalidTag(_))
    ));
}
