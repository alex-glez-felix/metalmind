use std::fs;

use metalmind_search::{Case, LineMatch, TextSearcher};
use metalmind_vault::VaultPath;
use pretty_assertions::assert_eq;

use super::*;

fn path(text: &str) -> VaultPath {
    VaultPath::new(text).unwrap()
}

/// Una nota con una línea que coincide por cada `(línea, texto)`: la
/// coincidencia es la primera `idea` del texto.
fn note(name: &str, lines: &[(usize, &str)]) -> NoteMatches {
    NoteMatches {
        path: path(name),
        lines: lines
            .iter()
            .map(|&(line, text)| {
                let start = text.find("idea").unwrap();
                LineMatch {
                    line,
                    offset: 0,
                    text: text.to_owned(),
                    matches: std::iter::once(start..start + 4).collect(),
                }
            })
            .collect(),
    }
}

/// Cada fila como `ruta:línea`, desde 1 como la muestra el picker.
fn rows(state: &TextSearchState) -> Vec<String> {
    (0..state.len())
        .map(|row| {
            let hit = state.hit(row).unwrap();
            format!("{}:{}", hit.path, hit.line.line + 1)
        })
        .collect()
}

fn typed(state: &mut TextSearchState, text: &str) -> Search {
    state.edit(LineEdit::Insert(text)).unwrap()
}

#[test]
fn cada_patron_es_una_busqueda_regex_con_su_version() {
    let mut state = TextSearchState::new();
    assert_eq!(state.status(), Status::Idle);
    let search = typed(&mut state, "id.a");
    assert_eq!(search.query.pattern, "id.a");
    assert_eq!(search.query.syntax, Syntax::Regex);
    assert_eq!(search.query.case, Case::Smart);
    assert_eq!(state.status(), Status::Searching);
    let next = typed(&mut state, "s");
    assert!(next.version > search.version);
    assert!(
        state.edit(LineEdit::Left).is_none(),
        "mover el cursor no busca"
    );
}

#[test]
fn las_notas_llegan_en_orden_de_llegada_y_las_viejas_se_descartan() {
    let mut state = TextSearchState::new();
    let old = typed(&mut state, "ide").version;
    let current = typed(&mut state, "a").version;
    let ignored = state.receive(old, vec![note("vieja.md", &[(0, "idea")])]);
    assert_eq!(ignored, Received::default());
    assert!(state.is_empty());
    let _ = state.receive(current, vec![note("b.md", &[(2, "una idea"), (7, "idea")])]);
    let _ = state.receive(current, vec![note("a.md", &[(0, "idea")])]);
    assert_eq!(rows(&state), ["b.md:3", "b.md:8", "a.md:1"]);
    state.finished(current);
    assert_eq!(state.status(), Status::Done);
}

/// La lista no parpadea vacía en cada tecla: lo de antes se ve hasta que
/// llega lo nuevo, y la fila marcada vuelve a la primera.
#[test]
fn lo_anterior_se_ve_hasta_que_llega_lo_nuevo() {
    let mut state = TextSearchState::new();
    let first = typed(&mut state, "idea").version;
    let _ = state.receive(first, vec![note("a.md", &[(0, "idea"), (1, "idea")])]);
    state.select_next();
    let second = typed(&mut state, "s").version;
    assert_eq!(rows(&state), ["a.md:1", "a.md:2"]);
    assert_eq!(state.selected(), 1);
    let _ = state.receive(second, vec![note("b.md", &[(4, "ideas")])]);
    assert_eq!(rows(&state), ["b.md:5"]);
    assert_eq!(state.selected(), 0);
}

#[test]
fn una_busqueda_sin_resultados_vacia_la_lista_al_terminar() {
    let mut state = TextSearchState::new();
    let first = typed(&mut state, "idea").version;
    let _ = state.receive(first, vec![note("a.md", &[(0, "idea")])]);
    state.finished(first);
    let second = typed(&mut state, "zzz").version;
    assert_eq!(rows(&state), ["a.md:1"]);
    state.finished(first);
    assert_eq!(rows(&state), ["a.md:1"], "terminar la vieja no cuenta");
    state.finished(second);
    assert!(state.is_empty());
    assert_eq!(state.status(), Status::Done);
}

/// Un regex a medio escribir no borra nada: la búsqueda anterior sigue.
#[test]
fn un_patron_invalido_no_busca_y_lo_anterior_sigue_llegando() {
    let mut state = TextSearchState::new();
    let valid = typed(&mut state, "idea").version;
    let _ = state.receive(valid, vec![note("a.md", &[(0, "idea")])]);
    assert!(state.edit(LineEdit::Insert("(")).is_none());
    assert!(matches!(state.status(), Status::Invalid(error) if error.contains("unclosed group")));
    let _ = state.receive(valid, vec![note("b.md", &[(0, "idea")])]);
    assert_eq!(rows(&state), ["a.md:1", "b.md:1"]);
    let fixed = typed(&mut state, ")").version;
    assert!(fixed > valid);
    assert_eq!(state.status(), Status::Searching);
}

#[test]
fn borrar_todo_vacia_la_lista_y_cancela_la_busqueda() {
    let mut state = TextSearchState::new();
    let first = typed(&mut state, "idea").version;
    let _ = state.receive(first, vec![note("a.md", &[(0, "idea")])]);
    let empty = state.edit(LineEdit::DeleteToStart).unwrap();
    assert_eq!(empty.query.pattern, "");
    assert!(state.is_empty());
    assert_eq!(state.status(), Status::Idle);
    let _ = state.receive(first, vec![note("b.md", &[(0, "idea")])]);
    assert!(state.is_empty());
}

/// Como en `space f`: lo que se teclea tras `enter` no cambia lo elegido.
#[test]
fn enter_antes_de_los_resultados_elige_el_primero_que_llega() {
    let mut state = TextSearchState::new();
    let first = typed(&mut state, "ide").version;
    let _ = state.receive(first, vec![note("vieja.md", &[(0, "idea")])]);
    let current = typed(&mut state, "a").version;
    assert_eq!(state.choose(Open::SplitRight), None);
    assert!(state.edit(LineEdit::Insert("zzz")).is_none());
    let received = state.receive(current, vec![note("b.md", &[(1, "idea")])]);
    let chosen = Chosen {
        item: 0,
        open: Open::SplitRight,
    };
    assert_eq!(received.chosen, Some(chosen));
    assert_eq!(rows(&state), ["b.md:2"]);
    assert_eq!(state.line().text(), "idea");
}

#[test]
fn enter_sin_resultados_no_elige_nada() {
    let mut state = TextSearchState::new();
    let version = typed(&mut state, "zzz").version;
    assert_eq!(state.choose(Open::Here), None);
    state.finished(version);
    assert_eq!(state.choose(Open::Here), None);
    assert!(
        state.edit(LineEdit::Insert("y")).is_some(),
        "se puede seguir"
    );
}

#[test]
fn elegir_y_moverse_por_la_lista() {
    let mut state = TextSearchState::new();
    let version = typed(&mut state, "idea").version;
    let _ = state.receive(version, vec![note("a.md", &[(0, "idea"), (1, "idea")])]);
    state.select_previous();
    assert_eq!(state.selected(), 1);
    state.select_next();
    assert_eq!(state.selected(), 0);
    state.select(1);
    state.select(9);
    let chosen = state.choose(Open::SplitDown).unwrap();
    assert_eq!(chosen.item, 1);
    assert_eq!(chosen.open, Open::SplitDown);
}

#[test]
fn con_demasiadas_lineas_se_corta_la_busqueda() {
    let mut state = TextSearchState::new();
    let version = typed(&mut state, "idea").version;
    let lines: Vec<(usize, &str)> = (0..MAX_HITS - 1).map(|line| (line, "idea")).collect();
    let received = state.receive(version, vec![note("a.md", &lines)]);
    assert!(!received.stop);
    let received = state.receive(version, vec![note("b.md", &[(0, "idea"), (1, "idea")])]);
    assert!(received.stop);
    assert_eq!(state.len(), MAX_HITS);
    assert_eq!(state.status(), Status::Truncated);
    let _ = state.receive(version, vec![note("c.md", &[(0, "idea")])]);
    state.finished(version);
    assert_eq!(state.len(), MAX_HITS);
    assert_eq!(state.status(), Status::Truncated);
}

#[test]
fn si_no_se_puede_buscar_lo_dice() {
    let mut state = TextSearchState::new();
    let version = typed(&mut state, "idea").version;
    state.failed(version, "sin hilos".to_owned());
    assert_eq!(state.status(), Status::Failed("sin hilos".to_owned()));
    assert_eq!(state.choose(Open::Here), None);
}

/// El modelo con el motor de verdad: lo que pide es lo que el motor busca.
#[test]
fn con_el_motor_de_metalmind_search() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "# Ideas\n\nuna Idea\nnada\n").unwrap();
    fs::create_dir(dir.path().join("d")).unwrap();
    fs::write(dir.path().join("d/b.md"), "otra idea, y otra idea\n").unwrap();
    fs::write(dir.path().join("c.txt"), "idea\n").unwrap();
    let mut state = TextSearchState::new();
    let search = typed(&mut state, "idea");
    let mut searcher = TextSearcher::new(dir.path());
    let found: Vec<NoteMatches> = searcher.search(&search.query).unwrap().iter().collect();
    let _ = state.receive(search.version, found);
    state.finished(search.version);
    let mut found = rows(&state);
    found.sort();
    assert_eq!(found, ["a.md:1", "a.md:3", "d/b.md:1"]);
    let b = (0..state.len())
        .map(|row| state.hit(row).unwrap())
        .find(|hit| hit.path.as_str() == "d/b.md")
        .unwrap();
    assert_eq!(b.line.matches, [5..9, 18..22]);
    assert_eq!(b.column(), 5);
}
