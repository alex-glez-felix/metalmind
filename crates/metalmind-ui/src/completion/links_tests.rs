use pretty_assertions::assert_eq;

use super::*;

fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Como las da el finder: las recientes primero, después por ruta.
fn vault() -> Arc<NoteCandidates> {
    candidates([
        "diario/2026-10-26.md",
        "proyectos/metalmind/plan.md",
        "aplanar.md",
        "ideas.md",
        "plantillas/planeación semanal.md",
    ])
}

fn candidates(names: impl IntoIterator<Item = &'static str>) -> Arc<NoteCandidates> {
    let notes: Arc<[Note]> = names.into_iter().map(|name| Note(path(name))).collect();
    Arc::new(NoteCandidates::new(notes))
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
    let more = candidates([
        "plan de viaje.md",
        "diario/2026-10-26.md",
        "proyectos/metalmind/plan.md",
        "aplanar.md",
        "ideas.md",
        "plantillas/planeación semanal.md",
    ]);
    let kept = list
        .with_candidates(Arc::clone(&more), &mut FuzzyMatcher::paths())
        .unwrap();
    assert_eq!(kept.accept(), &path("aplanar.md"));
    assert_eq!(kept.selection().selected(), 3);
    let fresh = open("plan")
        .unwrap()
        .with_candidates(more, &mut FuzzyMatcher::paths())
        .unwrap();
    assert_eq!(fresh.source(), &source);
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
    let source = path("ideas.md");
    assert!(list.is_for(&candidates, "pl", &source));
    assert!(!list.is_for(&candidates, "pla", &source));
    assert!(!list.is_for(&vault(), "pl", &source));
    assert!(
        !list.is_for(&candidates, "pl", &path("aplanar.md")),
        "desde otra nota es otra lista"
    );
}

/// Seguir escribiendo filtra solo lo que ya coincidía y da lo mismo que
/// abrirla de cero; borrar vuelve a mirar todas.
#[rstest::rstest]
#[case::sigue("pl", "plan")]
#[case::sigue_con_un_espacio("idea", "idea 1")]
#[case::otra_cosa("plan", "diario")]
#[case::borra("plan", "pl")]
#[case::desde_nada("", "aplanar")]
fn reescribir_da_lo_mismo_que_abrir(#[case] before: &str, #[case] after: &str) {
    let candidates = many();
    let source = path("ideas.md");
    let matcher = &mut FuzzyMatcher::paths();
    let opened = |typed: &str, matcher: &mut FuzzyMatcher| {
        LinkCompletions::open(Arc::clone(&candidates), typed, &source, matcher)
            .map(|list| list.rows().cloned().collect::<Vec<_>>())
    };
    let retyped = LinkCompletions::open(Arc::clone(&candidates), before, &source, matcher)
        .unwrap()
        .retyped(after, matcher)
        .map(|list| list.rows().cloned().collect::<Vec<_>>());
    assert_eq!(retyped, opened(after, matcher));
}

/// Muchas que empatan en puntaje y largo.
fn many() -> Arc<NoteCandidates> {
    let mut notes = vec![Note(path("aplanar.md")), Note(path("diario.md"))];
    for index in 0..40 {
        notes.push(Note(path(&format!("Ideas/idea {index:02} — plan.md"))));
        notes.push(Note(path(&format!("diario/{index:02}/plan.md"))));
    }
    Arc::new(NoteCandidates::new(Arc::from(notes)))
}

/// Un `!` o un `^` al principio de un nombre se buscan tal cual.
#[test]
fn lo_escrito_no_tiene_operadores() {
    let list = LinkCompletions::open(
        candidates(["!buzón.md", "buzón.md"]),
        "!bu",
        &path("otra.md"),
        &mut FuzzyMatcher::paths(),
    )
    .unwrap();
    assert_eq!(list.rows().collect::<Vec<_>>(), [&path("!buzón.md")]);
}
