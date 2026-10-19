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

/// Lo que se busca con `space /`: solo está en [`NOTE`], que se busca en
/// memoria y en orden. Con varias notas, los resultados llegan en el orden
/// en que terminan los hilos y la captura no se repetiría.
pub const SEARCHED: &str = "zeta";

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
const DEEP_FOLDER_2: &str =
    "Proyectos/Metalmind/Diseño/una carpeta más honda/y otra todavía más honda, con nombre largo";

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
            "# Idea {index}\n\nUna idea sobre {topic}: {}\n\nOtra línea con la idea repetida, idea tras idea.\n",
            "texto que sigue y sigue sin cortarse para que la línea pase del ancho de la ventana "
                .repeat(4)
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
        &format!("{DEEP_FOLDER_2}/la nota al fondo del árbol, con un nombre que tampoco cabe.md"),
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
    fs::create_dir_all(root.join("Vacía")).expect("crear la carpeta vacía");
}

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
