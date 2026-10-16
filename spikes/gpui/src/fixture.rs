//! Markdown sintético y determinista para probar archivos grandes: prosa con
//! acentos, listas, código, tablas, wikilinks, líneas muy largas y algo de
//! emoji y CJK para forzar fuentes de respaldo.

use std::io::{self, Write};

const WORDS: &[&str] = &[
    "nota",
    "árbol",
    "canción",
    "niño",
    "pingüino",
    "año",
    "corazón",
    "jardín",
    "mañana",
    "ventana",
    "teclado",
    "índice",
    "vínculo",
    "búsqueda",
    "último",
    "rápido",
    "señal",
    "cámara",
    "lápiz",
    "música",
    "bóveda",
    "enlace",
    "párrafo",
    "título",
    "código",
    "función",
    "acción",
    "edición",
    "cursor",
    "línea",
    "palabra",
    "texto",
    "archivo",
    "carpeta",
    "mensaje",
    "idea",
    "proyecto",
    "reunión",
    "río",
    "montaña",
    "café",
    "pequeño",
    "grande",
    "también",
    "después",
    "aquí",
    "allí",
    "según",
    "quizá",
    "el",
    "la",
    "los",
    "las",
    "de",
    "en",
    "con",
    "para",
    "por",
    "que",
    "y",
    "o",
    "un",
    "una",
    "sobre",
    "entre",
    "hacia",
];
const LINKS: &[&str] = &[
    "Índice general",
    "Diario 2026-10-15",
    "Ideas sueltas",
    "Lecturas pendientes",
    "Proyecto nota",
    "Recetas de la abuela",
];
const TAGS: &[&str] = &[
    "#idea",
    "#pendiente",
    "#lectura",
    "#vim",
    "#rust",
    "#diseño",
];
const CODE: &[&str] = &[
    "fn main() {",
    "    let buffer = Buffer::from(\"hola\");",
    "    for línea in texto.lines() {",
    "        println!(\"{línea}\");",
    "    }",
    "    let total: usize = valores.iter().sum();",
    "    match modo { Modo::Normal => {}, Modo::Insert => {} }",
    "}",
];
const EXOTIC: &[&str] = &[
    "📝 Nota rápida con emoji: ✅ hecho, ⏳ pendiente, 🔥 urgente",
    "日本語のメモ：これは長いファイルのテストです。",
    "Ελληνικά και русский текст para probar fuentes de respaldo",
    "Fórmula: ∑ xᵢ² ≤ ∞, flechas → ← ↑ ↓, guion — y comillas «así»",
];

const FLUSH_BYTES: usize = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub bytes: u64,
    pub lines: u64,
}

/// Escribe markdown hasta superar `target_bytes`, siempre terminando en `\n`.
///
/// # Errors
/// Los de escritura en `out`.
pub fn generate(out: &mut impl Write, target_bytes: u64) -> io::Result<Stats> {
    let mut writer = Writer::new(out);
    writer.push(
        "---\ntitle: Archivo de prueba de nota\ntags: [spike, rendimiento]\n---\n\n# Archivo de prueba\n\n",
    )?;
    let mut rng = Rng(0x6e6f_7461);
    let mut block = 0_u64;
    while writer.stats.bytes < target_bytes {
        block += 1;
        let text = if block % 12 == 1 {
            heading(&mut rng, block / 12 + 1)
        } else {
            body_block(&mut rng)
        };
        writer.push(&text)?;
    }
    writer.finish()
}

fn body_block(rng: &mut Rng) -> String {
    match rng.below(100) {
        0..45 => paragraph(rng, 3, 7),
        45..60 => list(rng, "- "),
        60..68 => tasks(rng),
        68..78 => code(rng),
        78..84 => quote(rng),
        84..90 => table(rng),
        90..93 => long_line(rng),
        93..97 => format!("{}\n\n", rng.pick(EXOTIC)),
        _ => list(rng, "1. "),
    }
}

fn heading(rng: &mut Rng, section: u64) -> String {
    format!("## Sección {section}: {}\n\n", capitalized(&words(rng, 3)))
}

fn paragraph(rng: &mut Rng, min_sentences: usize, max_sentences: usize) -> String {
    let count = min_sentences + rng.below(max_sentences - min_sentences + 1);
    let sentences: Vec<String> = (0..count).map(|_| sentence(rng)).collect();
    format!("{}\n\n", sentences.join(" "))
}

fn sentence(rng: &mut Rng) -> String {
    let count = 6 + rng.below(11);
    let text = capitalized(&words(rng, count));
    let suffix = match rng.below(10) {
        0 => format!(" [[{}]]", rng.pick(LINKS)),
        1 => format!(" {}", rng.pick(TAGS)),
        2 => format!(" **{}**", rng.pick(WORDS)),
        3 => format!(" `{}`", rng.pick(WORDS)),
        _ => String::new(),
    };
    format!("{text}{suffix}.")
}

fn list(rng: &mut Rng, marker: &str) -> String {
    let items = 3 + rng.below(5);
    block(rng, items, |rng| {
        let count = 3 + rng.below(8);
        format!("{marker}{}\n", capitalized(&words(rng, count)))
    })
}

fn tasks(rng: &mut Rng) -> String {
    let items = 2 + rng.below(4);
    block(rng, items, |rng| {
        let mark = if rng.below(2) == 0 { ' ' } else { 'x' };
        format!("- [{mark}] {}\n", capitalized(&words(rng, 4)))
    })
}

fn code(rng: &mut Rng) -> String {
    let lines = 3 + rng.below(6);
    let body: Vec<&str> = (0..lines).map(|_| *rng.pick(CODE)).collect();
    format!("```rust\n{}\n```\n\n", body.join("\n"))
}

fn quote(rng: &mut Rng) -> String {
    format!("> {}", paragraph(rng, 1, 3))
}

fn table(rng: &mut Rng) -> String {
    let rows = block(rng, 4, |rng| {
        let (word, value) = (rng.pick(WORDS), rng.below(10_000));
        format!("| {word} | {value} | {} |\n", words(rng, 3))
    });
    format!("| Campo | Valor | Nota |\n|---|---|---|\n{rows}")
}

/// `count` líneas hechas por `line`, seguidas de una línea en blanco.
fn block(rng: &mut Rng, count: usize, mut line: impl FnMut(&mut Rng) -> String) -> String {
    let mut text: String = (0..count).map(|_| line(rng)).collect();
    text.push('\n');
    text
}

/// Un párrafo de varios miles de caracteres en una sola línea, como los que
/// deja el soft wrap.
fn long_line(rng: &mut Rng) -> String {
    paragraph(rng, 40, 80)
}

fn words(rng: &mut Rng, count: usize) -> String {
    (0..count)
        .map(|_| *rng.pick(WORDS))
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// `SplitMix64`: suficiente para variar el texto y reproducible entre corridas.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).unwrap_or(u64::MAX).max(1);
        usize::try_from(self.next() % bound).unwrap_or_default()
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

struct Writer<'a, W: Write> {
    out: &'a mut W,
    pending: String,
    stats: Stats,
}

impl<'a, W: Write> Writer<'a, W> {
    fn new(out: &'a mut W) -> Self {
        Self {
            out,
            pending: String::with_capacity(FLUSH_BYTES * 2),
            stats: Stats { bytes: 0, lines: 0 },
        }
    }

    fn push(&mut self, text: &str) -> io::Result<()> {
        self.pending.push_str(text);
        self.stats.bytes += text.len() as u64;
        self.stats.lines += text.matches('\n').count() as u64;
        if self.pending.len() >= FLUSH_BYTES {
            self.out.write_all(self.pending.as_bytes())?;
            self.pending.clear();
        }
        Ok(())
    }

    fn finish(self) -> io::Result<Stats> {
        self.out.write_all(self.pending.as_bytes())?;
        self.out.flush()?;
        Ok(self.stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(target: u64) -> (String, Stats) {
        let mut bytes = Vec::new();
        let stats = generate(&mut bytes, target).unwrap();
        (String::from_utf8(bytes).unwrap(), stats)
    }

    #[test]
    fn alcanza_el_tamano_pedido_y_termina_en_salto_de_linea() {
        let (text, stats) = sample(200_000);
        assert!(text.len() >= 200_000);
        assert_eq!(stats.bytes, text.len() as u64);
        assert_eq!(stats.lines, text.matches('\n').count() as u64);
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn es_determinista() {
        assert_eq!(sample(50_000).0, sample(50_000).0);
    }

    #[test]
    fn incluye_cada_tipo_de_bloque() {
        let (text, _) = sample(300_000);
        for needle in [
            "## Sección",
            "```rust",
            "- [",
            "| Campo |",
            "> ",
            "[[",
            "1. ",
            "日本語",
        ] {
            assert!(text.contains(needle), "falta {needle:?}");
        }
    }

    #[test]
    fn incluye_lineas_muy_largas() {
        let (text, _) = sample(300_000);
        let longest = text.lines().map(str::len).max().unwrap();
        assert!(longest > 4_000, "la línea más larga mide {longest} bytes");
    }
}
