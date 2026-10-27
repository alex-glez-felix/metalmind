use metalmind_search::{FuzzyMatcher, Haystacks, Query, Ranked};
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::rstest;

const VAULT: [&str; 6] = [
    "diario/2026-10-17.md",
    "proyectos/nota/ideas.md",
    "proyectos/nota/vault.md",
    "recetas/arroz.md",
    "ideas.md",
    "img/vault.png",
];

fn ranked(query: &str) -> Vec<&'static str> {
    FuzzyMatcher::paths()
        .rank(&Query::new(query), &VAULT)
        .into_iter()
        .map(|found| VAULT[found.index])
        .collect()
}

/// Lo que coincide marcado entre corchetes, para leer las posiciones.
fn highlighted(query: &str, candidate: &str) -> String {
    let positions = FuzzyMatcher::paths().positions(&Query::new(query), candidate);
    if positions.is_empty() {
        return "(nada)".to_owned();
    }
    let mut out = String::new();
    for (offset, c) in candidate.char_indices() {
        if positions.contains(&offset) {
            out.push('[');
            out.push(c);
            out.push(']');
        } else {
            out.push(c);
        }
    }
    out
}

#[rstest]
#[case("ideas", &["ideas.md", "proyectos/nota/ideas.md"])]
#[case("vault", &["img/vault.png", "proyectos/nota/vault.md"])]
#[case("pnv", &["proyectos/nota/vault.md"])]
#[case("vault md", &["proyectos/nota/vault.md"])]
#[case("arroz", &["recetas/arroz.md"])]
#[case("zzz", &[])]
fn ordena_por_puntaje(#[case] query: &str, #[case] expected: &[&str]) {
    assert_eq!(ranked(query), expected);
}

#[test]
fn el_query_vacio_devuelve_todo_en_orden() {
    assert_eq!(ranked(""), VAULT);
}

#[test]
fn solo_espacios_es_un_query_vacio() {
    assert!(Query::new("   ").is_empty());
    assert_eq!(ranked("  "), VAULT);
}

#[test]
fn sin_coincidencia_no_hay_posiciones() {
    assert_eq!(highlighted("zzz", "ideas.md"), "(nada)");
}

#[test]
fn el_texto_suelto_no_da_bonus_de_ruta() {
    let query = Query::new("ab");
    let candidates = ["xa/b", "ab/x"];
    let ranked = FuzzyMatcher::text().rank(&query, &candidates);
    assert_eq!(ranked[0].index, 1);
}

#[test]
fn mayusculas_solo_si_el_query_las_lleva() {
    let count = |query: &str, candidate: &str| {
        FuzzyMatcher::paths()
            .rank(&Query::new(query), &[candidate])
            .len()
    };
    assert_eq!(count("readme", "README.md"), 1);
    assert_eq!(count("README", "readme.md"), 0);
}

#[test]
fn acentos_solo_si_el_query_los_lleva() {
    let count = |query: &str, candidate: &str| {
        FuzzyMatcher::paths()
            .rank(&Query::new(query), &[candidate])
            .len()
    };
    assert_eq!(count("cancion", "canción.md"), 1);
    assert_eq!(count("canción", "canción.md"), 1);
    assert_eq!(count("canción", "cancion.md"), 0);
}

#[rstest]
#[case("pnv", "proyectos/nota/vault.md", "[p]royectos/[n]ota/[v]ault.md")]
#[case(
    "vault md",
    "proyectos/nota/vault.md",
    "proyectos/nota/[v][a][u][l][t].[m][d]"
)]
#[case("cancion", "música/canción.md", "música/[c][a][n][c][i][ó][n].md")]
#[case("niño", "años/niño.md", "años/[n][i][ñ][o].md")]
#[case("nino", "años/niño.md", "años/[n][i][ñ][o].md")]
#[case("e", "cafe\u{301}/e.md", "cafe\u{301}/[e].md")]
fn las_posiciones_son_bytes(#[case] query: &str, #[case] candidate: &str, #[case] expected: &str) {
    assert_eq!(highlighted(query, candidate), expected);
}

#[test]
fn las_posiciones_caen_en_fronteras_de_caracter() {
    let candidate = "señal/ñandú.md";
    let positions = FuzzyMatcher::paths().positions(&Query::new("nandu"), candidate);
    assert_ne!(positions, [0usize; 0]);
    for &offset in &positions {
        assert!(candidate.is_char_boundary(offset), "{offset}");
    }
}

proptest! {
    /// Un query sacado del propio candidato, letra a letra y en orden, siempre
    /// coincide; y sus posiciones se pueden usar para cortar el texto.
    #[test]
    fn una_subsecuencia_siempre_coincide(
        candidate in "[a-zñáéíóú/ .-]{1,30}",
        picks in prop::collection::vec(any::<bool>(), 30),
    ) {
        let query: String = candidate
            .chars()
            .zip(&picks)
            .filter(|(c, pick)| **pick && c.is_alphanumeric() && c.is_ascii())
            .map(|(c, _)| c)
            .collect();
        let query_text = query;
        let query = Query::new(&query_text);
        let matcher = &mut FuzzyMatcher::paths();
        prop_assert_eq!(matcher.rank(&query, &[candidate.as_str()]).len(), 1);
        let positions = &matcher.positions(&query, &candidate);
        prop_assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        prop_assert!(positions.iter().all(|&offset| candidate.is_char_boundary(offset)));
        prop_assert_eq!(positions.len(), query_text.chars().count());
    }
}

/// Un prefijo sigue las reglas de mayúsculas y acentos del fuzzy.
#[test]
fn prefijo_con_las_reglas_del_fuzzy() {
    let candidates = ["canción", "Cancionero", "la canción", "cañón"];
    let starts = |text: &str| -> Vec<&str> {
        let mut found: Vec<&str> = FuzzyMatcher::text()
            .rank(&Query::prefix(text), &candidates)
            .into_iter()
            .map(|found| candidates[found.index])
            .collect();
        found.sort_unstable();
        found
    };
    assert_eq!(starts("cancion"), ["Cancionero", "canción"]);
    assert_eq!(starts("canción"), ["canción"]);
    assert_eq!(starts("cañ"), ["cañón"]);
}

/// Lo literal no tiene operadores: `!inbox` busca el `!`, `nota$` el `$`, y
/// un espacio es parte de lo que se busca.
#[test]
fn literal_sin_operadores() {
    let candidates = [
        "!inbox",
        "inbox",
        "nota$ cara",
        "nota barata",
        "^raíz",
        "plan/diario",
        "plan de viaje",
        "a\\ b",
    ];
    let found = |text: &str| -> Vec<&str> {
        FuzzyMatcher::text()
            .rank(&Query::literal(text), &candidates)
            .into_iter()
            .map(|found| candidates[found.index])
            .collect()
    };
    assert_eq!(found("!inb"), ["!inbox"]);
    assert_eq!(found("nota$"), ["nota$ cara"]);
    assert_eq!(found("^ra"), ["^raíz"]);
    assert_eq!(found("ta c"), ["nota$ cara"]);
    assert_eq!(found("plan d"), ["plan de viaje"], "el espacio se busca");
    assert_eq!(found("a\\ "), ["a\\ b"], "la barra también");
    assert_eq!(found("").len(), candidates.len());
}

/// Sobre candidatos ya convertidos da lo mismo que sobre los textos, y con
/// una parte de ellos, lo mismo que con esa parte.
#[test]
fn convertidos_dan_lo_mismo() {
    let candidates = [
        "notas/canción de cuna",
        "Canciones/la canción",
        "cañón",
        "日本語のノート/canción",
        "plan",
        "planes/canción",
    ];
    let haystacks = Haystacks::new(candidates);
    let all: Vec<usize> = (0..candidates.len()).collect();
    for text in ["", "can", "cancion", "Can", "ñ", "zz", "plan"] {
        for query in [Query::literal(text), Query::prefix(text)] {
            let matcher = &mut FuzzyMatcher::paths();
            assert_eq!(
                matcher.rank_among(&query, &haystacks, &all),
                matcher.rank(&query, &candidates),
                "{text}"
            );
            let some = [1, 3, 5];
            let expected: Vec<Ranked> = matcher
                .rank(&query, &some.map(|index| candidates[index]))
                .into_iter()
                .map(|ranked| Ranked {
                    index: some[ranked.index],
                    ..ranked
                })
                .collect();
            assert_eq!(
                matcher.rank_among(&query, &haystacks, &some),
                expected,
                "{text}"
            );
        }
    }
}

/// Un prefijo con espacios es uno solo, no uno por palabra.
#[test]
fn prefijo_con_espacios() {
    let candidates = ["idea 101", "idea suelta 1", "1 idea"];
    let found: Vec<&str> = FuzzyMatcher::text()
        .rank(&Query::prefix("idea 1"), &candidates)
        .into_iter()
        .map(|found| candidates[found.index])
        .collect();
    assert_eq!(found, ["idea 101"]);
}
