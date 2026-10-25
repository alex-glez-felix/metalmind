use std::sync::Arc;

use metalmind_search::FuzzyMatcher;
use pretty_assertions::assert_eq;

use super::*;

const NOTES: [&str; 5] = [
    "diario/hoy.md",
    "ideas.md",
    "proyectos/nota/ideas.md",
    "recetas/arroz.md",
    "recetas/pan.md",
];

fn picker() -> PickerState<&'static str> {
    PickerState::new(Arc::from(NOTES))
}

fn rows(picker: &PickerState<&'static str>) -> Vec<&'static str> {
    (0..picker.len())
        .map(|row| *picker.item(row).unwrap())
        .collect()
}

/// Escribe `text` y corre en el momento cada búsqueda que pida.
fn type_and_run(picker: &mut PickerState<&'static str>, text: &str) {
    let mut job = picker.edit(LineEdit::Insert(text)).job;
    while let Some(pending) = job {
        job = picker.finish(pending.run(&mut FuzzyMatcher::paths())).job;
    }
}

#[test]
fn sin_query_muestra_todo_en_orden_sin_buscar() {
    let mut picker = picker();
    assert_eq!(rows(&picker), NOTES);
    assert!(picker.is_current());
    type_and_run(&mut picker, "arroz");
    assert_eq!(rows(&picker), ["recetas/arroz.md"]);
    assert!(picker.edit(LineEdit::DeleteToStart).job.is_none());
    assert_eq!(rows(&picker), NOTES);
}

#[test]
fn filtra_y_ordena_con_metalmind_search() {
    let mut picker = picker();
    type_and_run(&mut picker, "ideas");
    assert_eq!(rows(&picker), ["ideas.md", "proyectos/nota/ideas.md"]);
    let positions = picker.positions(1, &mut FuzzyMatcher::paths());
    assert_eq!(positions, [15, 16, 17, 18, 19]);
    assert_eq!(picker.positions(9, &mut FuzzyMatcher::paths()), [0usize; 0]);
}

#[test]
fn mover_el_cursor_no_busca() {
    let mut picker = picker();
    type_and_run(&mut picker, "pan");
    assert!(picker.edit(LineEdit::Left).job.is_none());
    assert!(picker.edit(LineEdit::Home).job.is_none());
    assert!(picker.is_current());
}

#[test]
fn la_seleccion_da_la_vuelta() {
    let mut picker = picker();
    picker.select_previous();
    assert_eq!(picker.selected(), 4);
    picker.select_next();
    assert_eq!(picker.selected(), 0);
    picker.select_next();
    picker.select_next();
    assert_eq!(picker.selected(), 2);
    picker.select(4);
    assert_eq!(picker.selected(), 4);
    picker.select(9);
    assert_eq!(picker.selected(), 4);
}

#[test]
fn escribir_vuelve_a_la_primera_fila() {
    let mut picker = picker();
    picker.select(3);
    type_and_run(&mut picker, "recetas");
    assert_eq!(picker.selected(), 0);
}

#[test]
fn sin_resultados_no_hay_nada_que_elegir() {
    let mut picker = picker();
    type_and_run(&mut picker, "zzz");
    assert!(picker.is_empty());
    picker.select_next();
    picker.select_previous();
    assert_eq!(picker.choose(Open::Here, &mut FuzzyMatcher::paths()), None);
}

#[test]
fn elige_por_la_posicion_en_los_items() {
    let mut picker = picker();
    type_and_run(&mut picker, "recetas");
    assert_eq!(rows(&picker), ["recetas/pan.md", "recetas/arroz.md"]);
    picker.select_next();
    let chosen = picker
        .choose(Open::SplitRight, &mut FuzzyMatcher::paths())
        .unwrap();
    assert_eq!(NOTES[chosen.item], "recetas/arroz.md");
    assert_eq!(chosen.open, Open::SplitRight);
}

/// Las teclas que llegan mientras corre una búsqueda no lanzan otra: al
/// volver, se busca una sola vez lo último escrito.
#[test]
fn solo_corre_una_busqueda_y_al_terminar_la_ultima() {
    let mut picker = picker();
    let first = picker.edit(LineEdit::Insert("r")).job.unwrap();
    assert!(picker.edit(LineEdit::Insert("e")).job.is_none());
    assert!(picker.edit(LineEdit::Insert("c")).job.is_none());
    let step = picker.finish(first.run(&mut FuzzyMatcher::paths()));
    assert!(!picker.is_current(), "lo de `r` no se muestra");
    assert_eq!(rows(&picker), NOTES);
    let latest = step.job.unwrap();
    let step = picker.finish(latest.run(&mut FuzzyMatcher::paths()));
    assert!(step.job.is_none());
    assert!(picker.is_current());
    assert_eq!(
        rows(&picker),
        [
            "recetas/pan.md",
            "recetas/arroz.md",
            "proyectos/nota/ideas.md"
        ]
    );
}

/// Borrar todo mientras corre una búsqueda muestra todo ya; lo que vuelve
/// después es viejo y se descarta.
#[test]
fn una_busqueda_vieja_se_descarta() {
    let mut picker = picker();
    let job = picker.edit(LineEdit::Insert("pan")).job.unwrap();
    assert!(picker.edit(LineEdit::DeleteToStart).job.is_none());
    let step = picker.finish(job.run(&mut FuzzyMatcher::paths()));
    assert!(step.job.is_none());
    assert_eq!(rows(&picker), NOTES);
}

/// `enter` antes de que vuelva la búsqueda de lo escrito busca en el
/// momento y elige sobre eso; lo que vuelve después es viejo.
#[test]
fn elegir_antes_de_los_resultados_busca_en_el_momento() {
    let mut picker = picker();
    let job = picker.edit(LineEdit::Insert("arroz")).job.unwrap();
    let chosen = picker
        .choose(Open::Here, &mut FuzzyMatcher::paths())
        .unwrap();
    assert_eq!(NOTES[chosen.item], "recetas/arroz.md");
    assert_eq!(chosen.open, Open::Here);
    let step = picker.finish(job.run(&mut FuzzyMatcher::paths()));
    assert!(step.job.is_none());
    assert_eq!(step.chosen, None, "no elige dos veces");
}

/// Lo que se teclea después del `enter` no cambia lo elegido.
#[test]
fn elige_con_lo_escrito_al_dar_enter_aunque_corra_otra_busqueda() {
    let mut picker = picker();
    let first = picker.edit(LineEdit::Insert("rec")).job.unwrap();
    assert!(picker.edit(LineEdit::Insert("etas/p")).job.is_none());
    let chosen = picker
        .choose(Open::SplitDown, &mut FuzzyMatcher::paths())
        .unwrap();
    assert_eq!(NOTES[chosen.item], "recetas/pan.md");
    assert!(picker.edit(LineEdit::Insert("zzz")).job.is_none());
    let step = picker.finish(first.run(&mut FuzzyMatcher::paths()));
    assert_eq!(step.chosen, None);
}

#[test]
fn nuevos_items_rehacen_la_busqueda_y_conservan_la_fila() {
    let mut picker = picker();
    type_and_run(&mut picker, "md");
    picker.select(2);
    let more: Arc<[&str]> = Arc::from([NOTES.as_slice(), &["nueva.md"]].concat());
    let job = picker.set_items(more).job.unwrap();
    assert!(!picker.is_current());
    let step = picker.finish(job.run(&mut FuzzyMatcher::paths()));
    assert!(step.job.is_none());
    assert_eq!(picker.len(), 6);
    assert_eq!(picker.selected(), 2);
}

#[test]
fn si_la_lista_se_achica_la_fila_queda_dentro() {
    let mut picker = picker();
    picker.select(4);
    assert!(picker.set_items(Arc::from(["a.md", "b.md"])).job.is_none());
    assert_eq!(picker.selected(), 1);
}

/// Escribir y dar `enter` mientras se indexa el vault abre lo que coincide
/// en cuanto llegan las notas.
#[test]
fn mientras_carga_la_eleccion_espera_a_los_items() {
    let mut picker = PickerState::loading();
    assert!(!picker.is_current());
    let step = picker.edit(LineEdit::Insert("pan"));
    assert!(step.job.is_none(), "sin ítems no se busca");
    assert_eq!(picker.choose(Open::Here, &mut FuzzyMatcher::paths()), None);
    let step = picker.set_items(Arc::from(NOTES));
    let step = picker.finish(step.job.unwrap().run(&mut FuzzyMatcher::paths()));
    let chosen = step.chosen.unwrap();
    assert_eq!(NOTES[chosen.item], "recetas/pan.md");
}

/// Con el query vacío no hay búsqueda: la elección pendiente sale al llegar
/// los ítems.
#[test]
fn mientras_carga_sin_query_elige_la_primera_al_llegar() {
    let mut picker = PickerState::loading();
    assert_eq!(
        picker.choose(Open::SplitRight, &mut FuzzyMatcher::paths()),
        None
    );
    let step = picker.set_items(Arc::from(NOTES));
    assert!(step.job.is_none());
    let chosen = step.chosen.unwrap();
    assert_eq!(
        (NOTES[chosen.item], chosen.open),
        ("diario/hoy.md", Open::SplitRight)
    );
}

/// Deja solo las notas de `recetas/`.
struct Recetas;

impl Narrowing<&'static str> for Recetas {
    fn fuzzy<'t>(&self, text: &'t str) -> std::borrow::Cow<'t, str> {
        text.into()
    }

    fn filters(&self, _text: &str) -> bool {
        true
    }

    fn keep(&self, _text: &str, items: &[&'static str]) -> Vec<usize> {
        (0..items.len())
            .filter(|&index| items[index].starts_with("recetas/"))
            .collect()
    }

    fn warnings(&self, _text: &str) -> Vec<std::ops::Range<usize>> {
        Vec::new()
    }
}

/// Notas y tags nuevos del watcher piden una sola búsqueda, ya con los dos.
#[test]
fn items_y_filtro_nuevos_buscan_una_vez() {
    let mut picker = picker();
    type_and_run(&mut picker, "md");
    assert_eq!(picker.len(), NOTES.len());
    let more: Arc<[&str]> = Arc::from([NOTES.as_slice(), &["recetas/sopa.md"]].concat());
    let job = picker.set_source(more, Arc::new(Recetas)).job.unwrap();
    let step = picker.finish(job.run(&mut FuzzyMatcher::paths()));
    assert!(step.job.is_none());
    assert!(picker.is_current());
    let mut shown = rows(&picker);
    shown.sort_unstable();
    assert_eq!(
        shown,
        ["recetas/arroz.md", "recetas/pan.md", "recetas/sopa.md"]
    );
}
