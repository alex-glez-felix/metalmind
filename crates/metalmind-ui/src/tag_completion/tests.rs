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
    TagCompletions::open(vault(), typed, place, &mut FuzzyMatcher::text())
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
    assert_eq!(
        list.accept(),
        Accepted {
            typed: "rs".to_owned(),
            text: "Rust".to_owned(),
        }
    );
}

/// Si el nombre y un alias coinciden, se muestra por el nombre.
#[test]
fn por_el_nombre_antes_que_por_un_alias() {
    let candidates: Arc<[TagCompletion]> = Arc::from([tag("Rust", 1, &["rustlang"])]);
    let list = TagCompletions::open(
        candidates,
        "rust",
        TagPlace::Inline,
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
    assert_eq!(
        list.accept().text,
        metalmind_vault::tags::yaml_scalar("2024")
    );
    assert_ne!(list.accept().text, "2024");
}

#[test]
fn elegir_da_la_vuelta() {
    let mut list = open("ru", TagPlace::Inline).unwrap();
    list.select_previous();
    assert_eq!(list.selected(), 2);
    list.select_next();
    assert_eq!(list.selected(), 0);
    list.select_next();
    assert_eq!(list.accept().text, "rueda");
    list.select(10);
    assert_eq!(list.selected(), 2);
}
