//! El vault de las capturas: contenido que estresa el layout. Una nota de
//! más de 1000 líneas con líneas más largas que la ventana, nombres y rutas
//! larguísimos, multibyte y emoji, carpetas hondas, muchas notas que
//! coinciden con una búsqueda y una carpeta vacía.
//! Todo es fijo: la misma corrida da los mismos píxeles.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// La nota que se abre: está en la raíz porque `metalmind nota.md` abre el
/// vault de la carpeta de la nota.
pub const NOTE: &str = "estrés del layout — una nota con un nombre bastante largo 🦀.md";

/// Las 22 fórmulas del spike (08 §2), una que no compone y un bloque de
/// varias líneas al final.
pub const FORMULAS: &str = "formulas.md";

/// La nota de [`write_tables`].
pub const TABLES: &str = "tablas.md";
/// Las filas del registro de [`write_tables`]: más de mil, como una nota larga.
const TABLE_ROWS: usize = 1200;

/// Una nota de 1 300 líneas con fórmulas en párrafos largos y en bloques, y
/// una zona de prueba en la línea [`FORMULAS_ZONE`]: el cursor va dentro de
/// la fórmula con `aqui` y la línea de [`FORMULAS_BROKEN`] lleva fórmulas
/// rotas hasta pasar del borde.
pub const LONG_FORMULAS: &str = "formulas-larga.md";
pub const FORMULAS_ZONE: usize = 1170;
pub const FORMULAS_BROKEN: usize = FORMULAS_ZONE + 8;

/// Lo que se busca con `space /`: solo está en [`NOTE`], que se busca en
/// memoria y en orden. Con varias notas, los resultados llegan en el orden
/// en que terminan los hilos y la captura no se repetiría.
pub const SEARCHED: &str = "zeta";

/// Lo que se busca en la escena `buscar-cjk`: va justo detrás de un texto
/// en japonés, así que la fila lo recorta con una elipsis delante del CJK,
/// que entra con su fuente de respaldo.
pub const SEARCHED_AFTER_CJK: &str = "kanji";

/// Las últimas líneas con [`SEARCHED`], cada una con un texto en japonés de
/// otro largo delante: la elipsis cae en un carácter distinto en cada fila.
const CJK_LINES: std::ops::RangeInclusive<usize> = 28..=30;

/// Carpetas abiertas en el árbol, como las dejaría el usuario.
pub const EXPANDED: &[&str] = &[
    "Ideas",
    "Proyectos",
    "Proyectos/Metalmind",
    "Proyectos/Metalmind/Diseño",
    DEEP_FOLDER_1,
    DEEP_FOLDER_2,
    LONG_FOLDER,
];

const DEEP_FOLDER_1: &str = "Proyectos/Metalmind/Diseño/una carpeta más honda";
pub const DEEP_FOLDER_2: &str =
    "Proyectos/Metalmind/Diseño/una carpeta más honda/y otra todavía más honda, con nombre largo";

/// La nota de la ruta más larga, en [`DEEP_FOLDER_2`]: la del indicador del
/// autoguardado.
pub const DEEP_NOTE: &str = "la nota al fondo del árbol, con un nombre que tampoco cabe.md";

/// Líneas de la nota de más que las de estilos: una nota larga de verdad.
const LONG_NOTE_LINES: usize = 1200;

const LONG_FOLDER: &str = "Una carpeta con un nombre larguísimo que no cabe en el panel lateral";

const TOPICS: &[&str] = &[
    "arquitectura del núcleo",
    "ñandú y acentos áéíóú",
    "日本語のノート",
    "🦀 rust y emoji 🎉✨",
    "un nombre de nota tan largo que en el finder tiene que recortarse por la derecha",
    "café crème",
];

const STYLES: &str = include_str!("../../../metalmind-syntax/tests/fixtures/estilos.md");
const SPIKE_FORMULAS: &str = include_str!("../../../metalmind-perf/fixtures/22-formulas.md");

pub fn write(root: &Path) {
    let file = |path: &str, contents: &[u8]| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("la ruta tiene carpeta"))
            .expect("crear la carpeta");
        fs::write(path, contents).expect("escribir el archivo");
    };
    file(NOTE, note().as_bytes());
    file("Nota existente.md", b"# Nota existente\n");
    file("otra.md", b"Otra nota.\n");
    file("diagrama.png", b"\x89PNG\r\n");
    for index in 1..=120 {
        let topic = TOPICS[index % TOPICS.len()];
        let body = format!(
            "# Idea {index}\n\nUna idea sobre {topic}: {}\n\nOtra línea con la idea repetida, idea tras idea.\n\n{}\n",
            "texto que sigue y sigue sin cortarse para que la línea pase del ancho de la ventana "
                .repeat(4),
            idea_tags(index),
        );
        file(
            &format!("Ideas/idea {index:03} — {topic}.md"),
            body.as_bytes(),
        );
    }
    file(
        "Proyectos/Metalmind/Diseño/tokens del tema.md",
        "# Tokens\n\nUna idea de diseño.\n".as_bytes(),
    );
    file(
        "Proyectos/Metalmind/Diseño/principios.md",
        b"# Principios\n\nEl contenido manda: la idea es callarse.\n",
    );
    file(
        "Proyectos/Metalmind/notas sueltas.md",
        b"Una idea suelta.\n",
    );
    file(
        &format!("{DEEP_FOLDER_2}/{DEEP_NOTE}"),
        b"Una idea muy honda.\n",
    );
    file(
        &format!("{LONG_FOLDER}/y una nota que tampoco cabe aunque la ventana sea ancha.md"),
        b"idea\n",
    );
    file(
        "Proyectos/foto de las vacaciones con un nombre larguísimo.jpg",
        b"\xff\xd8\xff",
    );
    // Las páginas dan la jerarquía y los íconos de la vista de tags.
    file(
        "tags/rust.md",
        "---\nicon: 🦀\naliases: [rs]\n---\nEl lenguaje.\n".as_bytes(),
    );
    file(
        "tags/diseño.md",
        "---\nicon: 🎨\nparents: [rust]\naliases: [arquitectura-de-la-interfaz-y-sus-principios-a-largo-plazo-con-un-alias-que-no-cabe-ni-en-la-ventana-mas-ancha-que-hay-porque-sigue-y-sigue-sin-parar-por-mucho-que-uno-quiera-leerlo-entero-de-una-sola-vez-en-la-lista-de-tags-del-autocompletado]\n---\n".as_bytes(),
    );
    fs::create_dir_all(root.join("Vacía")).expect("crear la carpeta vacía");
    file(FORMULAS, SPIKE_FORMULAS.as_bytes());
    file(LONG_FORMULAS, long_formulas().as_bytes());
}

/// Las plantillas: la de entrevista de `docs/plantillas.md`, otra que pide
/// `#rust`, una de nombre largo en subcarpetas y muchas para llenar el picker.
pub fn write_templates(root: &Path) {
    let file = |path: &str, contents: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("la ruta tiene carpeta"))
            .expect("crear la carpeta");
        fs::write(path, contents).expect("escribir el archivo");
    };
    file("plantillas/entrevista.md", interview());
    file(
        "plantillas/rust/capitulo.md",
        "# {{title}}\n\nEmpezado el {{date:dddd D [de] MMMM}}.\n\n## Idea\n\n## Ejemplos\n",
    );
    file(
        "plantillas/diario.md",
        "# {{date:dddd D [de] MMMM [de] YYYY}}\n\n## Pendientes\n\n## Notas\n",
    );
    file(
        "plantillas/trabajo/reuniones/una plantilla de reunion semanal con un nombre que no cabe en el picker.md",
        "# {{title}}\n\n## Asistentes\n\n## Acuerdos\n",
    );
    for topic in TOPICS {
        file(&format!("plantillas/temas/{topic}.md"), "# {{title}}\n\n");
    }
    file(
        "tags/rust.md",
        "---\nicon: 🦀\naliases: [rs]\ntemplate: rust/capitulo\n---\nEl lenguaje.\n",
    );
    file("tags/diario.md", "---\ntemplate: diario\n---\n");
}

/// Dos `plan.md` que empatan por nombre, una en la carpeta de nombre
/// larguísimo: `[[plan]]` es esa, la menos honda, y el link a la otra
/// lleva su carpeta.
pub fn write_link_notes(root: &Path) {
    for folder in ["Proyectos/Metalmind", LONG_FOLDER] {
        let path = root.join(folder).join("plan.md");
        fs::create_dir_all(path.parent().expect("la ruta tiene carpeta"))
            .expect("crear la carpeta");
        fs::write(path, "# Plan\n").expect("escribir el archivo");
    }
}

/// Una nota con tablas sin alinear, como quedan llenadas a mano: anchos en
/// columnas de pantalla (acentos, CJK, emoji), `|` que no parten celdas,
/// filas cortas y largas, una cita y un registro de [`TABLE_ROWS`] filas.
pub fn write_tables(root: &Path) {
    let mut note = String::from(concat!(
        "# Evaluación de la entrevista\n\n",
        "| Criterio | Peso | Nota | Comentario |\n",
        "|:-|:-:|-:|-|\n",
        "| Comunicación clara | 2 | 4 | explicó el diseño sin jerga |\n",
        "| Código | 3 | 5 | `match a | b` en los patrones, y `\\|` escapado |\n",
        "| Diseño de sistemas | 3 | 3 | [[entrevistas/2026/octubre/notas del panel|ver notas]] |\n",
        "| 日本語の資料 | 1 | 🦀 | un comentario larguísimo que no cabe en la ventana ni alineado ni sin alinear, ",
        "porque sigue y sigue con detalles de la entrevista que nadie va a leer completos |\n",
        "| Total |\n",
        "| Extra | 1 | 2 | 3 | sobra | y otra más |\n\n",
        "> | Cita | En una cita |\n> |-|-|\n> | dentro | sigue en la cita |\n\n",
        "## Registro\n\n",
        "| # | Fecha | Evento | Detalle |\n|---|:-:|---|--:|\n",
    ));
    for row in 1..=TABLE_ROWS {
        let event = ["alta", "revisión de seguimiento", "baja", "nota"][row % 4];
        let detail = "x".repeat(row % 37);
        let day = row % 28 + 1;
        writeln!(note, "| {row} | 2026-10-{day:02} | {event} | {detail} |").expect("escribir");
    }
    note.push_str("| fin | | evento-final | |\n");
    fs::write(root.join(TABLES), note).expect("escribir la nota de tablas");
}

/// Notas que enlazan a `Proyectos/Metalmind/plan.md`: muchas, en una
/// carpeta honda, con líneas más largas que el panel, un link a un heading,
/// multibyte y alguna con varios links.
pub fn write_backlinks(root: &Path) {
    let file = |path: &str, contents: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("la ruta tiene carpeta"))
            .expect("crear la carpeta");
        fs::write(path, contents).expect("escribir el archivo");
    };
    for index in 1..=24 {
        let topic = TOPICS[index % TOPICS.len()];
        let mut body = format!(
            "# Reunión {index:02}\n\nAcordamos revisar el [[Metalmind/plan]] antes del viernes, \
             con todo lo que quedó pendiente de la semana pasada y lo de {topic}.\n"
        );
        if index % 3 == 0 {
            body.push_str(
                "\n日本語のメモ:計画を見直す [[Metalmind/plan#Meta]] とても長い行が続きます。\n",
            );
        }
        if index % 5 == 0 {
            body.push_str("\nY otra vez: [[Metalmind/plan|el plan]] 🦀 al final.\n");
        }
        file(
            &format!("{DEEP_FOLDER_1}/reuniones/reunión {index:02} — {topic}.md"),
            &body,
        );
    }
}

/// Una nota con muchos links que no llevan a nada: un nombre larguísimo,
/// japonés, emoji, un embed y un link markdown.
pub fn write_unresolved(root: &Path) {
    let mut body = String::from(
        "# Pendientes\n\n\
         - [[Una nota que todavía no existe y tiene un nombre larguísimo para ver cómo se recorta en el picker]]\n\
         - ![[diagrama-que-falta.png]]\n\
         - [el informe final](informes/2026/informe-final.md)\n\
         - [[日本語のノート/未作成のメモ]]\n\
         - [[🦀 rust/capítulo 9 — lifetimes]]\n",
    );
    for index in 1..=30 {
        writeln!(
            body,
            "- [[tarea pendiente {index:02}]] para la semana que viene"
        )
        .expect("escribir en un String");
    }
    let path = root
        .join(LONG_FOLDER)
        .join("pendientes de una lista que tampoco cabe.md");
    fs::write(path, body).expect("escribir el archivo");
}

/// La plantilla de ejemplo, tal como la muestra la documentación.
fn interview() -> &'static str {
    let docs = include_str!("../../../../docs/plantillas.md");
    let start = docs
        .find("```markdown\n")
        .expect("el ejemplo de la documentación")
        + 12;
    let end = start + docs[start..].find("```\n").expect("el cierre del ejemplo");
    &docs[start..end]
}

/// Los tags de una idea: unas sin ninguno (el buzón), otras con varios, y
/// uno con un nombre que no cabe.
fn idea_tags(index: usize) -> String {
    [
        (2, "#Rust"),
        (3, "#diseño"),
        (5, "#archivado"),
        (
            7,
            "#un-tag-con-un-nombre-larguísimo-para-ver-dónde-se-corta",
        ),
    ]
    .iter()
    .filter(|(every, _)| index.is_multiple_of(*every))
    .map(|(_, tag)| *tag)
    .collect::<Vec<_>>()
    .join(" ")
}

/// Párrafos de más de 250 caracteres con fórmulas en línea, un bloque de
/// varias líneas y uno solo en su línea cada tanto, y la zona de prueba.
fn long_formulas() -> String {
    let mut text = String::new();
    let mut line = 1;
    while line <= 1300 {
        if line == FORMULAS_ZONE {
            text.push_str(FORMULAS_ZONE_TEXT);
            text.push_str("Rotas:");
            text.push_str(&" texto $\\frac{1}{$".repeat(24));
            text.push('\n');
            line += FORMULAS_BROKEN - FORMULAS_ZONE + 1;
        } else if line % 50 == 0 {
            writeln!(text, "## Sección {}", line / 50).expect("escribir");
            line += 1;
        } else if line % 50 == 17 && !(line..line + 4).contains(&FORMULAS_ZONE) {
            write!(
                text,
                "$$\n\\begin{{aligned}} f_{{{line}}}(x) &= (x+1)^2 \\\\\n&= x^2 + 2x + 1 \\end{{aligned}}\n$$\n"
            )
            .expect("escribir");
            line += 4;
        } else if line % 50 == 33 {
            writeln!(text, "$$\\int_0^{{{line}}} e^{{-t^2}}\\,dt$$").expect("escribir");
            line += 1;
        } else {
            writeln!(
                text,
                "Párrafo {line}: la energía es $E_{{{line}}} = mc^2$ y el texto sigue con palabras y más palabras, luego $\\frac{{a_{{{line}}}}}{{b_{{{line}}}}} + \\sqrt{{{line}}}$ y continúa hasta pasar del borde de la ventana, con $\\sum_{{k=1}}^{{{line}}} k$ al final."
            )
            .expect("escribir");
            line += 1;
        }
    }
    text
}

/// Las líneas de la zona de prueba antes de la de fórmulas rotas.
const FORMULAS_ZONE_TEXT: &str = "### Zona de prueba
El cursor está dentro de $\\alpha_{aqui} + \\beta^2 = \\gamma$ y esa línea muestra la fuente; el párrafo sigue largo, con $\\sqrt{x_{1170}^2 + y^2}$ en medio y más texto que pasa del borde de la ventana.
Un párrafo largo con $\\frac{\\partial f}{\\partial x}$, $\\sum_{k=1}^{n} k^2 = \\frac{n(n+1)(2n+1)}{6}$ y $e^{i\\theta}$, y texto que sigue y sigue hasta pasar del borde.
$$
\\begin{pmatrix} a & b \\\\
c & d \\end{pmatrix} \\begin{pmatrix} x \\\\ y \\end{pmatrix} = \\begin{pmatrix} ax + by \\\\ cx + dy \\end{pmatrix}
$$
Otro párrafo con $\\int_0^1 x^2\\,dx = \\frac{1}{3}$ y $\\lim_{n \\to \\infty} (1 + 1/n)^n = e$ entre palabras que siguen hasta el borde.
";

fn note() -> String {
    let long_word = "palabrasinespaciosqueno".repeat(12);
    let mut long_tail = String::new();
    for line in 1..=LONG_NOTE_LINES {
        let filler = if line % 25 == 0 {
            "una línea larga en medio de la nota larga, que pasa del ancho de la ventana y sigue "
                .repeat(3)
        } else {
            String::new()
        };
        writeln!(long_tail, "Línea {line} de la nota larga. {filler}").expect("escribir");
    }
    let mut searched = String::new();
    for line in 1..=30 {
        if CJK_LINES.contains(&line) {
            let cjk: String = "ヴィンとケルシエはルサデルの霧の中を歩き、長い夜を過ごした。"
                .chars()
                .cycle()
                .take(40 + 7 * (line - CJK_LINES.start()))
                .collect();
            writeln!(
                searched,
                "{cjk}{SEARCHED_AFTER_CJK} · {SEARCHED} {line}: ñandú 🦀"
            )
            .expect("escribir");
            continue;
        }
        let filler = "una coincidencia en una línea más larga que el picker ".repeat(line % 4);
        writeln!(searched, "{SEARCHED} {line}: {filler} ñandú 日本語 🦀").expect("escribir");
    }
    format!(
        "{STYLES}\n\
         ## Un heading largo que no cabe en la ventana de 1280 ni recortando los márgenes del panel\n\n\
         Una línea muy larga, más que el ancho de la ventana, para ver qué pasa en el borde derecho del editor cuando el texto no cabe y sigue y sigue: {}\n\
         {long_word}\n\
         Un link larguísimo: https://example.com/una/ruta/muy/larga/que/no/termina/nunca?con=parametros&y=mas&parametros=todavia\n\n\
         Multibyte: ñandú, café crème, 日本語のテキスト, 한국어, العربية, emoji 🦀🎉✨👩‍💻 y combinados e\u{301}.\n\
         \tUna línea con tab al principio.\n\n\
         {searched}\n\
         {long_tail}",
        "y otra frase más que alarga la línea ".repeat(6)
    )
}
