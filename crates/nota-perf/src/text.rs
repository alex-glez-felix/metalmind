//! Generadores deterministas: la misma llamada da siempre el mismo texto, en
//! cualquier máquina, para que los conteos de instrucciones sean comparables.

use std::fmt::Write as _;

/// Palabra frecuente en [`note`] (cientos de veces cada mil líneas): la que buscan y
/// sustituyen los benchmarks.
pub const WORD: &str = "nota";

/// Palabra que nunca sale en [`note`]: buscarla recorre todo el buffer.
pub const MISSING_WORD: &str = "zzzz";

const WORDS: [&str; 24] = [
    "el",
    "vault",
    "guarda",
    WORD,
    "en",
    "markdown",
    "con",
    "links",
    "tags",
    "y",
    "frontmatter",
    "cada",
    "línea",
    "tiene",
    "texto",
    "corrido",
    "sin",
    "acentos",
    "raros",
    "pero",
    "también",
    "ñandú",
    "café",
    "año",
];

/// Generador lineal congruente: suficiente para variar el texto y sin
/// depender de un crate de aleatorios cuya secuencia podría cambiar.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Los 32 bits altos son los de mejor calidad en un LCG.
        usize::try_from(self.0 >> 32).unwrap_or(0)
    }

    fn below(&mut self, n: usize) -> usize {
        self.next() % n
    }
}

/// Una nota markdown de `lines` líneas con lo que tiene una nota de verdad:
/// frontmatter, encabezados, párrafos, listas con wikilinks y tags, links
/// markdown, código inline y bloques cercados.
#[must_use]
pub fn note(lines: usize) -> String {
    let mut rng = Lcg(0x6e6f_7461);
    let mut out = Lines::new(lines);
    out.push("---");
    out.push("tags: [proyecto, lectura/pendiente]");
    out.push("aliases: [Nota grande]");
    out.push("---");
    let mut line = String::new();
    let mut index = 0;
    while !out.is_full() {
        line.clear();
        match index % 100 {
            0 => {
                let _ = write!(line, "## Sección {}", index / 100);
            }
            40 => {
                out.push("```rust");
                out.push("let enlace = \"[[no es un link]]\"; // #no-es-tag");
                line.push_str("```");
            }
            n if n % 7 == 0 => {
                let _ = write!(
                    line,
                    "- [ ] tarea {index} sobre [[{WORD}-{}]] #area/{}",
                    rng.below(500),
                    rng.below(20)
                );
            }
            n if n % 13 == 0 => {}
            _ => paragraph_line(&mut line, &mut rng, index),
        }
        out.push(&line);
        index += 1;
    }
    out.text
}

/// Texto que deja de crecer al llegar a `limit` líneas.
struct Lines {
    text: String,
    written: usize,
    limit: usize,
}

impl Lines {
    fn new(limit: usize) -> Self {
        Self {
            text: String::with_capacity(limit * 64),
            written: 0,
            limit,
        }
    }

    fn is_full(&self) -> bool {
        self.written == self.limit
    }

    fn push(&mut self, line: &str) {
        if !self.is_full() {
            self.text.push_str(line);
            self.text.push('\n');
            self.written += 1;
        }
    }
}

fn paragraph_line(line: &mut String, rng: &mut Lcg, index: usize) {
    let words = 8 + rng.below(8);
    for word in 0..words {
        if word > 0 {
            line.push(' ');
        }
        match rng.below(40) {
            0 => {
                let _ = write!(line, "[[{WORD}-{}|alias]]", rng.below(500));
            }
            1 => {
                let _ = write!(line, "#tag{}", rng.below(50));
            }
            2 => {
                let _ = write!(line, "[otra](carpeta/{WORD}-{index}.md)");
            }
            3 => line.push_str("`código`"),
            _ => line.push_str(WORDS[rng.below(WORDS.len())]),
        }
    }
}

/// `count` líneas con rachas de `#` cada vez más largas: encabezados
/// imposibles, tags vacíos y `##` pegados a texto.
#[must_use]
pub fn hash_runs(count: usize) -> String {
    let mut text = String::new();
    for n in 0..count {
        let run = 1 + n % 64;
        text.push_str(&"#".repeat(run));
        text.push_str(if n % 2 == 0 { " x\n" } else { "x#y##\n" });
    }
    text
}

/// Una sola línea con `count` wikilinks sin cerrar (`[[a [[b …`).
#[must_use]
pub fn unclosed_wikilinks(count: usize) -> String {
    let mut text = String::new();
    for n in 0..count {
        let _ = write!(text, "[[{WORD}-{n} ");
    }
    text
}

/// Una sola línea con `count` links markdown cuyo título `(` no cierra.
#[must_use]
pub fn unclosed_link_titles(count: usize) -> String {
    "[a](b (".repeat(count)
}

/// Rachas sin pareja de largos 1, 2, …, `count` en un solo párrafo.
#[must_use]
pub fn backtick_runs(count: usize) -> String {
    (1..=count).map(|len| "`".repeat(len) + " ").collect()
}

/// Una línea de unos `bytes` bytes sin ningún `\n`, con links y tags.
#[must_use]
pub fn huge_line(bytes: usize) -> String {
    let mut rng = Lcg(0x6c69_6e65);
    let mut text = String::with_capacity(bytes + 64);
    let mut index = 0;
    while text.len() < bytes {
        paragraph_line(&mut text, &mut rng, index);
        text.push(' ');
        index += 1;
    }
    text
}

/// Rutas de un vault de `notes` notas en 10 × 10 carpetas, más un adjunto por
/// carpeta: la forma del vault sintético de `nota-vault/benches/open.rs`.
#[must_use]
pub fn vault_paths(notes: usize) -> Vec<String> {
    let per_folder = notes.div_ceil(100);
    let mut paths = Vec::with_capacity(notes + 100);
    for top in 0..10 {
        for sub in 0..10 {
            for note in 0..per_folder {
                paths.push(format!("area-{top}/tema-{sub}/{WORD}-{note}.md"));
            }
            paths.push(format!("area-{top}/tema-{sub}/imagen.png"));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_has_exactly_the_requested_lines() {
        for lines in [0, 1, 3, 4, 5, 41, 140, 5_000] {
            assert_eq!(note(lines).lines().count(), lines, "{lines}");
        }
    }

    #[test]
    fn note_is_deterministic() {
        assert_eq!(note(500), note(500));
        assert_eq!(huge_line(10_000), huge_line(10_000));
    }

    #[test]
    fn note_contains_the_word_and_never_the_missing_one() {
        let text = note(1_000);
        assert!(text.matches(WORD).count() > 500);
        assert!(!text.contains(MISSING_WORD));
    }

    #[test]
    fn huge_line_has_no_line_feed() {
        let line = huge_line(100_000);
        assert!(line.len() >= 100_000);
        assert!(!line.contains('\n'));
    }

    #[test]
    fn vault_paths_counts_notes_and_attachments() {
        assert_eq!(vault_paths(5_000).len(), 5_100);
    }
}
