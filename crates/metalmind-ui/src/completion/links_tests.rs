use pretty_assertions::assert_eq;

use super::*;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Como las da el finder: las recientes primero, después por ruta.
fn vault() -> Arc<[Note]> {
    [
        "diario/2026-10-26.md",
        "proyectos/metalmind/plan.md",
        "aplanar.md",
        "ideas.md",
        "plantillas/planeación semanal.md",
    ]
    .into_iter()
    .map(|name| Note(path(name)))
    .collect()
}

fn open(typed: &str) -> Option<LinkCompletions> {
    LinkCompletions::open(
        vault(),
        typed,
        &path("ideas.md"),
        &mut FuzzyMatcher::paths(),
    )
}

fn shown(typed: &str) -> Vec<String> {
    open(typed)
        .map(|list| list.rows().map(|path| path.as_str().to_owned()).collect())
        .unwrap_or_default()
}

/// Con nada escrito, todas en el orden del finder, sin la nota de donde
/// sale el link.
#[test]
fn con_nada_escrito_todas_menos_la_propia() {
    assert_eq!(
        shown(""),
        [
            "diario/2026-10-26.md",
            "proyectos/metalmind/plan.md",
            "aplanar.md",
            "plantillas/planeación semanal.md",
        ]
    );
}

/// Primero las que tienen un nombre que empieza con lo escrito (también
/// sin acento), después lo demás que coincide en la ruta.
#[test]
fn los_nombres_que_empiezan_con_lo_escrito_van_primero() {
    let shown = shown("plan");
    assert_eq!(shown.len(), 3);
    assert_eq!(shown[2], "aplanar.md");
    assert!(shown[..2].contains(&"proyectos/metalmind/plan.md".to_owned()));
    assert!(shown[..2].contains(&"plantillas/planeación semanal.md".to_owned()));
    assert_eq!(
        shown_first("planeacion"),
        "plantillas/planeación semanal.md"
    );
}

fn shown_first(typed: &str) -> String {
    shown(typed).swap_remove(0)
}

/// La ruta cuenta: `metal/plan` encuentra la nota por sus carpetas, y la
/// extensión `.md` no se escribe.
#[test]
fn por_la_ruta_sin_la_extension() {
    assert_eq!(shown("metal/plan"), ["proyectos/metalmind/plan.md"]);
    assert_eq!(shown("ideas.md"), Vec::<String>::new());
}

#[test]
fn sin_coincidencias_no_hay_lista() {
    assert!(open("zzz").is_none());
}

/// Aceptar da la nota elegida; elegir da la vuelta.
#[test]
fn elegir_da_la_vuelta_y_aceptar_da_la_nota() {
    let mut list = open("plan").unwrap();
    list.selection_mut().select_previous();
    assert_eq!(list.accept(), &path("aplanar.md"));
    list.selection_mut().select_next();
    assert_eq!(list.selection().selected(), 0);
}

/// Un lote del vault (otra nota nueva, el autoguardado) deja elegida la
/// misma nota aunque cambie de fila; sin elegir, la mejor coincidencia.
#[test]
fn notas_nuevas_dejan_elegida_la_misma() {
    let source = path("ideas.md");
    let mut list = open("plan").unwrap();
    list.selection_mut().select(2);
    let mut more = vault().to_vec();
    more.insert(0, Note(path("plan de viaje.md")));
    let more: Arc<[Note]> = Arc::from(more);
    let kept = list
        .with_candidates(Arc::clone(&more), &source, &mut FuzzyMatcher::paths())
        .unwrap();
    assert_eq!(kept.accept(), &path("aplanar.md"));
    assert_eq!(kept.selection().selected(), 3);
    let fresh = open("plan")
        .unwrap()
        .with_candidates(more, &source, &mut FuzzyMatcher::paths())
        .unwrap();
    assert_eq!(fresh.selection().selected(), 0);
}

#[test]
fn es_la_misma_lista_solo_con_lo_mismo() {
    let candidates = vault();
    let list = LinkCompletions::open(
        Arc::clone(&candidates),
        "pl",
        &path("ideas.md"),
        &mut FuzzyMatcher::paths(),
    )
    .unwrap();
    assert!(list.is_for(&candidates, "pl"));
    assert!(!list.is_for(&candidates, "pla"));
    assert!(!list.is_for(&vault(), "pl"));
}
