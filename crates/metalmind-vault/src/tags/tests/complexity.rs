//! Lo que recorre todo el vault crece lineal con las notas (docs/performance.md):
//! N contra 16N notas, con una página de tag cada 25.

use super::Fixture;
use crate::tags::{Scope, TagQuery, Tags};

const NOTES: usize = 2_000;

fn fixture(notes: usize) -> Fixture {
    let files = metalmind_perf::tagged_vault(notes, notes / 25);
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    Fixture::new(&files)
}

fn tags(notes: usize) -> Tags {
    fixture(notes).tags()
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn armar_los_tags_es_lineal() {
    metalmind_perf::assert_linear(NOTES, fixture, |fixture| fixture.tags());
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn consultar_es_lineal() {
    let query = TagQuery::parse("#t1 -#t3").query;
    metalmind_perf::assert_linear(NOTES, tags, |tags| query.notes(&tags).len());
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn armar_el_panel_es_lineal() {
    metalmind_perf::assert_linear(NOTES, tags, |tags| {
        tags.tree(&Scope::default().filter(&tags)).roots().len()
    });
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn armar_el_panel_en_focus_es_lineal() {
    let scope = Scope::parse("area-1/ #t1").unwrap();
    metalmind_perf::assert_linear(NOTES, tags, |tags| {
        tags.tree(&scope.filter(&tags)).roots().len()
    });
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn los_candidatos_de_autocompletado_son_lineales() {
    metalmind_perf::assert_linear(NOTES, tags, |tags| tags.completions().len());
}

#[test]
#[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
fn planear_un_renombre_es_lineal() {
    metalmind_perf::assert_linear(NOTES, tags, |tags| {
        tags.plan_rename("#t1", "#uno")
            .unwrap()
            .changes
            .edit_count()
    });
}
