use std::fs;

use metalmind_buffer::Buffer;
use pretty_assertions::assert_eq;

use super::*;

/// El resultado de la línea `line` (desde 0) de `path`, con `text` y lo que
/// coincide como `(inicio, fin)`.
fn hit(path: &str, line: usize, text: &str, matches: &[(usize, usize)]) -> Hit {
    Hit {
        path: VaultPath::new(path).unwrap(),
        line: LineMatch {
            line,
            offset: 0,
            text: text.to_owned(),
            matches: matches.iter().map(|&(start, end)| start..end).collect(),
        },
    }
}

/// Cada línea como `número:texto`, con `[` `]` alrededor de lo resaltado y
/// `>` en la del resultado.
fn shown(preview: &Preview) -> Vec<String> {
    let Preview::Lines { lines, focus } = preview else {
        panic!("sin líneas: {preview:?}");
    };
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let mut text = line.text.clone();
            for range in line.highlights.iter().rev() {
                text.insert(range.end, ']');
                text.insert(range.start, '[');
            }
            let mark = if Some(index) == *focus { ">" } else { "" };
            format!("{mark}{}:{text}", line.number)
        })
        .collect()
}

fn numbered(count: usize) -> String {
    (1..=count)
        .map(|n| format!("l{n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn toma_las_lineas_alrededor_y_resalta_la_del_resultado() {
    let text = "uno\nuna idea\r\ntres\n";
    let preview = excerpt(text, &hit("a.md", 1, "una idea", &[(4, 8)]));
    assert_eq!(shown(&preview), ["1:uno", ">2:una [idea]", "3:tres"]);
}

#[test]
fn en_una_nota_larga_toma_solo_las_de_alrededor() {
    let text = numbered(50_000);
    let preview = excerpt(&text, &hit("a.md", 25_000, "l25001", &[(0, 2)]));
    let lines = shown(&preview);
    assert_eq!(lines.len(), 2 * AROUND + 1);
    assert_eq!(
        lines[0],
        format!("{}:l{}", 25_001 - AROUND, 25_001 - AROUND)
    );
    assert_eq!(lines[AROUND], ">25001:[l2]5001");
}

/// Al principio de la nota no hay líneas antes: van más de después.
#[test]
fn al_principio_de_la_nota_toma_las_de_despues() {
    let preview = excerpt(&numbered(100), &hit("a.md", 0, "l1", &[(0, 2)]));
    let lines = shown(&preview);
    assert_eq!(lines[0], ">1:[l1]");
    assert_eq!(lines.len(), 2 * AROUND + 1);
}

/// Si la nota cambió desde la búsqueda, lo que coincidía ya no está ahí.
#[test]
fn una_linea_que_cambio_no_se_resalta() {
    let preview = excerpt("otra cosa\n", &hit("a.md", 0, "una idea", &[(4, 8)]));
    assert_eq!(shown(&preview), [">1:otra cosa"]);
}

#[test]
fn una_nota_que_ya_no_llega_a_la_linea_lo_dice() {
    let preview = excerpt("uno\n", &hit("a.md", 40, "x", &[(0, 1)]));
    assert_eq!(
        preview,
        Preview::Message("La nota ya no llega a esa línea".into())
    );
}

#[test]
fn una_coincidencia_lejos_del_principio_se_ve() {
    let line = format!("{}idea al final", "x".repeat(200));
    let preview = excerpt(&line, &hit("a.md", 0, &line, &[(200, 204)]));
    let expected = format!(">1:…{}[idea] al final", "x".repeat(BEFORE_MATCH));
    assert_eq!(shown(&preview), [expected]);
}

/// Un carácter CJK ocupa dos columnas: la línea se corta antes.
#[test]
fn una_coincidencia_tras_texto_ancho_tambien_se_ve() {
    let line = format!("{}kanji", "霧".repeat(20));
    let start = line.len() - "kanji".len();
    let preview = excerpt(&line, &hit("a.md", 0, &line, &[(start, line.len())]));
    let expected = format!(">1:…{}[kanji]", "霧".repeat(BEFORE_MATCH / 2));
    assert_eq!(shown(&preview), [expected]);
}

#[test]
fn una_linea_enorme_se_corta() {
    let text = format!("{}\nidea\n", "é".repeat(1 << 20));
    let preview = excerpt(&text, &hit("a.md", 1, "idea", &[(0, 4)]));
    let lines = shown(&preview);
    assert_eq!(lines[0], format!("1:{}…", "é".repeat(MAX_LINE)));
    assert_eq!(lines[1], ">2:[idea]");
}

#[test]
fn los_tabs_se_ven_como_espacios() {
    let preview = excerpt("\tidea\n", &hit("a.md", 0, "\tidea", &[(1, 5)]));
    assert_eq!(shown(&preview), [">1: [idea]"]);
}

fn vault(notes: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in notes {
        fs::write(dir.path().join(name), text).unwrap();
    }
    dir
}

fn load(state: &mut PreviewState, hit: &Hit) -> Option<Preview> {
    let job = state.request(Some(hit))?;
    state.finish(job.run());
    state.shown().cloned()
}

#[test]
fn lee_la_nota_del_disco() {
    let dir = vault(&[("a.md", "uno\nuna idea\n")]);
    let mut state = PreviewState::new(dir.path().to_owned(), &[]);
    let preview = load(&mut state, &hit("a.md", 1, "una idea", &[(4, 8)])).unwrap();
    assert_eq!(shown(&preview), ["1:uno", ">2:una [idea]"]);
}

/// Lo que se ve de una nota con cambios sin guardar es el buffer.
#[test]
fn una_nota_abierta_con_cambios_se_ve_como_en_el_editor() {
    let dir = vault(&[("a.md", "en disco\n")]);
    let open = [OpenNote {
        path: VaultPath::new("a.md").unwrap(),
        text: Buffer::from("sin guardar\nuna idea").snapshot(),
    }];
    let mut state = PreviewState::new(dir.path().to_owned(), &open);
    let preview = load(&mut state, &hit("a.md", 1, "una idea", &[(4, 8)])).unwrap();
    assert_eq!(shown(&preview), ["1:sin guardar", ">2:una [idea]"]);
}

/// Moverse entre los resultados de una misma nota no la vuelve a leer.
#[test]
fn otro_resultado_de_la_misma_nota_no_la_lee_de_nuevo() {
    let dir = vault(&[("a.md", "idea uno\nidea dos\n")]);
    let mut state = PreviewState::new(dir.path().to_owned(), &[]);
    load(&mut state, &hit("a.md", 0, "idea uno", &[(0, 4)])).unwrap();
    fs::remove_file(dir.path().join("a.md")).unwrap();
    let preview = load(&mut state, &hit("a.md", 1, "idea dos", &[(0, 4)])).unwrap();
    assert_eq!(shown(&preview), ["1:idea uno", ">2:[idea] dos"]);
}

#[test]
fn el_mismo_resultado_no_se_pide_dos_veces() {
    let dir = vault(&[("a.md", "idea\n")]);
    let mut state = PreviewState::new(dir.path().to_owned(), &[]);
    let found = hit("a.md", 0, "idea", &[(0, 4)]);
    assert!(state.request(Some(&found)).is_some());
    assert!(state.request(Some(&found)).is_none());
    let other_pattern = hit("a.md", 0, "idea", &[(0, 2)]);
    assert!(
        state.request(Some(&other_pattern)).is_some(),
        "con otro patrón cambia lo resaltado"
    );
}

/// Lo de un pedido viejo no se muestra, y lo de antes se ve hasta que
/// llega lo nuevo.
#[test]
fn lo_que_vuelve_de_un_pedido_viejo_se_descarta() {
    let dir = vault(&[("a.md", "idea a\n"), ("b.md", "idea b\n")]);
    let mut state = PreviewState::new(dir.path().to_owned(), &[]);
    load(&mut state, &hit("a.md", 0, "idea a", &[(0, 4)])).unwrap();
    let old = state
        .request(Some(&hit("b.md", 0, "idea b", &[(0, 4)])))
        .unwrap();
    let new = state
        .request(Some(&hit("a.md", 0, "idea a", &[(0, 2)])))
        .unwrap();
    assert_eq!(
        shown(state.shown().unwrap()),
        [">1:[idea] a"],
        "lo de antes"
    );
    state.finish(old.run());
    assert_eq!(shown(state.shown().unwrap()), [">1:[idea] a"]);
    state.finish(new.run());
    assert_eq!(shown(state.shown().unwrap()), [">1:[id]ea a"]);
}

#[test]
fn sin_resultado_el_panel_queda_vacio() {
    let dir = vault(&[("a.md", "idea\n")]);
    let mut state = PreviewState::new(dir.path().to_owned(), &[]);
    let pending = state
        .request(Some(&hit("a.md", 0, "idea", &[(0, 4)])))
        .unwrap();
    assert!(state.request(None).is_none());
    state.finish(pending.run());
    assert_eq!(state.shown(), None);
}

#[test]
fn si_no_se_puede_leer_lo_dice() {
    let dir = vault(&[]);
    let mut state = PreviewState::new(dir.path().to_owned(), &[]);
    let preview = load(&mut state, &hit("nada.md", 0, "idea", &[(0, 4)])).unwrap();
    assert!(
        matches!(&preview, Preview::Message(text) if text.starts_with("No se pudo leer la nota")),
        "{preview:?}"
    );
}
