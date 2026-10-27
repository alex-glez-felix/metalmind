use pretty_assertions::assert_eq;

use super::*;

fn tag(name: &str, count: usize, aliases: &[&str]) -> TagCompletion {
    TagCompletion {
        key: TagKey::new(name),
        name: name.to_owned(),
        aliases: aliases.iter().map(|&alias| alias.to_owned()).collect(),
        count,
        color: None,
        icon: None,
    }
}

/// Como los da `Tags::completions`: de más notas a menos.
fn vault() -> Arc<[TagCompletion]> {
    Arc::from([
        tag("arquitectura", 30, &[]),
        tag("Rust", 20, &["rs"]),
        tag("rueda", 5, &[]),
        tag("2024", 4, &[]),
        tag("gpui", 3, &[]),
    ])
}

fn open(typed: &str, place: TagPlace) -> Option<TagCompletions> {
    TagCompletions::open(vault(), typed, place, false, &mut FuzzyMatcher::text())
}

/// Cada fila como se ve: el nombre y, si coincidió por él, el alias.
fn shown(typed: &str) -> Vec<String> {
    open(typed, TagPlace::Inline)
        .map(|list| {
            list.rows()
                .map(|(completion, alias)| match alias {
                    Some(alias) => format!("{} ({alias})", completion.name),
                    None => completion.name.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Primero los que empiezan con lo escrito, de más notas a menos; después
/// lo demás que coincide.
#[test]
fn los_que_empiezan_con_lo_escrito_van_primero() {
    assert_eq!(shown("ru"), ["Rust", "rueda", "arquitectura"]);
    assert_eq!(shown("RU"), ["Rust", "rueda", "arquitectura"]);
}

/// `#rs` encuentra `Rust` por su alias, y aceptar deja la forma establecida.
#[test]
fn un_alias_encuentra_su_tag_y_acepta_su_nombre() {
    assert_eq!(shown("rs"), ["Rust (rs)"]);
    let list = open("rs", TagPlace::Inline).unwrap();
    assert_eq!(list.typed(), "rs");
    assert_eq!(list.accept(), "Rust");
}

/// Si el nombre y un alias coinciden, se muestra por el nombre.
#[test]
fn por_el_nombre_antes_que_por_un_alias() {
    let candidates: Arc<[TagCompletion]> = Arc::from([tag("Rust", 1, &["rustlang"])]);
    let list = TagCompletions::open(
        candidates,
        "rust",
        TagPlace::Inline,
        false,
        &mut FuzzyMatcher::text(),
    )
    .unwrap();
    assert_eq!(list.rows().next().unwrap().1, None);
}

#[test]
fn sin_coincidencias_no_hay_lista() {
    assert!(open("zzz", TagPlace::Inline).is_none());
}

/// En el YAML se escribe un escalar que se lea igual: `2024` sin comillas
/// sería un número.
#[test]
fn en_el_frontmatter_acepta_un_escalar_de_yaml() {
    let list = open("202", TagPlace::Tags).unwrap();
    assert_eq!(list.accept(), metalmind_vault::tags::yaml_scalar("2024"));
    assert_ne!(list.accept(), "2024");
}

#[test]
fn elegir_da_la_vuelta() {
    let mut list = open("ru", TagPlace::Inline).unwrap();
    list.selection_mut().select_previous();
    assert_eq!(list.selection().selected(), 2);
    list.selection_mut().select_next();
    assert_eq!(list.selection().selected(), 0);
    list.selection_mut().select_next();
    assert_eq!(list.accept(), "rueda");
    list.selection_mut().select(10);
    assert_eq!(list.selection().selected(), 2);
}

/// Solo es la misma lista con lo mismo escrito, en el mismo lugar y entre
/// los mismos tags: si el vault trae otros, se abre de nuevo.
#[test]
fn es_la_misma_lista_solo_con_lo_mismo() {
    let candidates = vault();
    let list = TagCompletions::open(
        Arc::clone(&candidates),
        "ru",
        TagPlace::Inline,
        false,
        &mut FuzzyMatcher::text(),
    )
    .unwrap();
    assert!(list.is_for(&candidates, "ru", TagPlace::Inline));
    assert!(!list.is_for(&candidates, "rue", TagPlace::Inline));
    assert!(!list.is_for(&candidates, "ru", TagPlace::Tags));
    assert!(!list.is_for(&vault(), "ru", TagPlace::Inline));
}

/// Un lote del vault trae otros tags con lo mismo escrito, como al
/// autoguardar la nota: sigue elegido el mismo tag aunque cambie de fila.
#[test]
fn tags_nuevos_dejan_elegido_el_mismo_tag() {
    let mut list = open("ru", TagPlace::Inline).unwrap();
    list.selection_mut().select_next();
    assert_eq!(list.accept(), "rueda");
    let mut more = vault().to_vec();
    more.insert(0, tag("ruido", 40, &[]));
    let list = list
        .with_candidates(Arc::from(more), false, &mut FuzzyMatcher::text())
        .unwrap();
    assert_eq!(
        (list.selection().selected(), list.accept()),
        (2, "rueda".to_owned())
    );
}

/// Sin una fila elegida por el usuario, queda la primera: la mejor
/// coincidencia entre los tags nuevos.
#[test]
fn sin_elegir_queda_la_mejor_coincidencia() {
    let list = open("ru", TagPlace::Inline).unwrap();
    let mut more = vault().to_vec();
    more.insert(0, tag("ruido", 40, &[]));
    let list = list
        .with_candidates(Arc::from(more), false, &mut FuzzyMatcher::text())
        .unwrap();
    assert_eq!(
        (list.selection().selected(), list.accept()),
        (0, "ruido".to_owned())
    );
}

/// Si el tag elegido ya no está, queda elegida la primera fila.
#[test]
fn sin_el_tag_elegido_vuelve_a_la_primera_fila() {
    let mut list = open("ru", TagPlace::Inline).unwrap();
    list.selection_mut().select_next();
    let without: Vec<TagCompletion> = vault()
        .iter()
        .filter(|completion| completion.name != "rueda")
        .cloned()
        .collect();
    let list = list
        .with_candidates(Arc::from(without), false, &mut FuzzyMatcher::text())
        .unwrap();
    assert_eq!(
        (list.selection().selected(), list.accept()),
        (0, "Rust".to_owned())
    );
}

/// Lo escrito y los nombres se pliegan igual, también cuando el plegado de
/// una letra son dos: `ß` es `ss`.
#[test]
fn el_plegado_de_varias_letras_coincide() {
    let candidates: Arc<[TagCompletion]> = Arc::from([tag("Groß", 2, &[]), tag("Strasse", 1, &[])]);
    let names = |typed: &str| -> Vec<String> {
        TagCompletions::open(
            Arc::clone(&candidates),
            typed,
            TagPlace::Inline,
            false,
            &mut FuzzyMatcher::text(),
        )
        .map(|list| list.rows().map(|(tag, _)| tag.name.clone()).collect())
        .unwrap_or_default()
    };
    assert_eq!(names("GROSS"), ["Groß"]);
    assert_eq!(names("straß"), ["Strasse"]);
}

/// Si el fuzzy encuentra `canción` con `cancion`, también cuenta como que
/// empieza con lo escrito: va antes que uno con menos notas.
#[test]
fn empezar_sin_acento_cuenta_como_empezar() {
    let candidates: Arc<[TagCompletion]> =
        Arc::from([tag("canción", 9, &[]), tag("cancionera", 1, &[])]);
    let list = TagCompletions::open(
        candidates,
        "cancion",
        TagPlace::Inline,
        false,
        &mut FuzzyMatcher::text(),
    )
    .unwrap();
    let names: Vec<&str> = list.rows().map(|(tag, _)| tag.name.as_str()).collect();
    assert_eq!(names, ["canción", "cancionera"]);
}

/// Lo que se escribe, si solo lo nombra esta ocurrencia ya autoguardada, no
/// se ofrece a sí mismo; los demás que coinciden sí.
#[test]
fn lo_escrito_no_se_ofrece_a_si_mismo() {
    let candidates: Arc<[TagCompletion]> = Arc::from([tag("ru", 1, &[]), tag("bruma", 1, &[])]);
    let names = |only_here: bool| -> Vec<String> {
        TagCompletions::open(
            Arc::clone(&candidates),
            "RU",
            TagPlace::Inline,
            only_here,
            &mut FuzzyMatcher::text(),
        )
        .map(|list| list.rows().map(|(tag, _)| tag.name.clone()).collect())
        .unwrap_or_default()
    };
    assert_eq!(names(false), ["ru", "bruma"]);
    assert_eq!(names(true), ["bruma"]);
}

/// La ventana de filas solo se corre cuando la elegida sale de ella: subir
/// desde abajo mueve la elegida por las que ya se ven.
#[test]
fn la_ventana_se_corre_solo_al_salir_la_elegida() {
    let mut list = open("", TagPlace::Inline).unwrap();
    assert_eq!(list.selection_mut().visible(3), 0..3);
    for _ in 0..4 {
        list.selection_mut().select_next();
    }
    assert_eq!(list.selection_mut().visible(3), 2..5);
    list.selection_mut().select_previous();
    list.selection_mut().select_previous();
    assert_eq!(
        (list.selection().selected(), list.selection_mut().visible(3)),
        (2, 2..5)
    );
    list.selection_mut().select_previous();
    assert_eq!(list.selection_mut().visible(3), 1..4);
    list.selection_mut().select_previous();
    list.selection_mut().select_previous();
    assert_eq!(
        (list.selection().selected(), list.selection_mut().visible(3)),
        (4, 2..5),
        "da la vuelta"
    );
    assert_eq!(list.selection_mut().visible(10), 0..5);
}
