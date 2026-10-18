use nota_search::{FuzzyMatcher, Query};
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
