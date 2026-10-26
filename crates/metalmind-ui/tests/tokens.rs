#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! Ningún tamaño ni color fijo en la UI fuera del módulo de tokens
//! (`src/theme.rs` y sus submódulos en `src/theme/`): todo sale del tema (`docs/design.md`, "Tokens"). Los
//! tests de cada archivo quedan fuera: miden con números a propósito.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

/// El módulo que traduce el tema a GPUI, con sus submódulos: el único que
/// escribe números.
const TOKENS: &str = "src/theme.rs";
const TOKENS_DIR: &str = "src/theme/";

/// Lo que no puede aparecer, con por qué.
const RULES: &[(&str, &str)] = &[
    // Cero no es una medida: es el tope de un `max`.
    (r"\bpx\(\s*-?(\d*[1-9]|0?\.\d*[1-9])", "píxeles literales"),
    (r"\brems\(", "tamaños literales en rems"),
    (
        r"\b(rgb|rgba|hsla|opaque_grey)\(|\bHsla\s*\{\s*h\s*:|\b(black|white|red|green|blue|yellow|transparent_black|transparent_white)\(\)",
        "colores literales",
    ),
    (r"\.opacity\(", "opacidades literales"),
    (r"\bFontWeight(::[A-Z_]+|\()", "pesos de letra literales"),
    (r"\.shadow", "sombras"),
    (
        r"\.(p|px|py|pt|pb|pl|pr|m|mx|my|mt|mb|ml|mr|gap|gap_x|gap_y|w|h|size|min_w|max_w|min_h|max_h|top|bottom|left|right|inset|rounded|border|border_t|border_b|border_l|border_r|border_x|border_y|text)_(\d+(p\d+)?|px|xs|sm|md|lg|xl|\dxl|full)\(\)",
        "tamaños fijos de los atajos de GPUI",
    ),
    (
        r"[*/]\s*\d+(\.\d*)?",
        "multiplicadores literales sobre una medida",
    ),
    (r"\brelative\(0\.\d", "fracciones literales"),
];

/// Atajos de GPUI que no son un tamaño de diseño: ocupar todo, o dejar
/// que un flex se encoja hasta cero.
const ALLOWED_SHORTCUTS: &[&str] = &[
    ".w_full()",
    ".h_full()",
    ".size_full()",
    ".min_w_0()",
    ".min_h_0()",
    ".inset_0()",
];

/// Excepciones: el archivo, lo que dice la línea y por qué vale.
const EXCEPTIONS: &[(&str, &str, &str)] = &[
    (
        "src/sidebar/render.rs",
        "gpui_kit::transparent_black()",
        "una fila sin marcar reserva su borde, transparente: si no, la fila brincaría un píxel al marcarse",
    ),
    (
        "src/editor/element.rs",
        "x * 2.",
        "geometría, no estilo: el margen del texto se resta de los dos lados",
    ),
    (
        "src/editor/element.rs",
        "y * 2.",
        "geometría, no estilo: el margen del texto se resta arriba y abajo",
    ),
    (
        "src/editor/markdown.rs",
        "spans.len() * 2",
        "la capacidad de un Vec (dos tramos por span y uno más), no una medida",
    ),
    (
        "src/math/geometry.rs",
        "value / 2.0",
        "geometría, no estilo: la mitad con que GPUI centra el texto en su línea y las fórmulas se centran",
    ),
    (
        "src/math/geometry.rs",
        "value * 2.0",
        "geometría, no estilo: el mismo margen a los dos lados",
    ),
    (
        "src/math/images.rs",
        "32 * 1024 * 1024",
        "el tope de la caché en bytes de memoria, no una medida de la UI",
    ),
    (
        "src/tag_change/view.rs",
        "relative(0.9)",
        "el mismo tope que el picker, sobre el que flota igual",
    ),
    (
        "src/picker/view/render.rs",
        "relative(0.9)",
        "tope del picker en una ventana angosta, como fracción de la ventana; es una pregunta abierta de la auditoría",
    ),
];

struct Finding {
    file: String,
    line: usize,
    text: String,
    /// Lo que violó la regla: una excepción cubre solo esto, no la línea.
    hit: String,
    rule: &'static str,
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn is_test_file(path: &str) -> bool {
    path.ends_with("_tests.rs") || path.ends_with("/tests.rs") || path.ends_with("/proptests.rs")
}

/// Las líneas de código de un archivo, sin comentarios de línea y hasta su
/// módulo de tests; un `#[cfg(test)]` sobre otra cosa no corta el archivo.
fn code_lines(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    let end = (0..lines.len())
        .find(|&index| {
            lines[index].trim_start().starts_with("#[cfg(test)]")
                && opens_a_module(&lines[index + 1..])
        })
        .unwrap_or(lines.len());
    lines[..end]
        .iter()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect()
}

/// Si lo que sigue a un atributo, saltando otros atributos, es un `mod`.
fn opens_a_module(rest: &[&str]) -> bool {
    rest.iter()
        .map(|line| line.trim_start())
        .find(|line| !line.starts_with("#["))
        .is_some_and(|line| line.starts_with("mod ") || line.starts_with("pub mod "))
}

/// Lo que viola una regla en el código de la UI, sin contar los tests.
fn findings(root: &Path) -> Vec<Finding> {
    let rules: Vec<(Regex, &str)> = RULES
        .iter()
        .map(|&(pattern, rule)| (Regex::new(pattern).unwrap(), rule))
        .collect();
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    files.sort();
    let mut found = Vec::new();
    for path in files {
        let file = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if file == TOKENS || file.starts_with(TOKENS_DIR) || is_test_file(&file) {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let lines = code_lines(&text);
        // Todo el archivo de una vez: un valor partido en varias líneas también cuenta.
        let code = lines.join("\n");
        for (regex, rule) in &rules {
            let hits = regex
                .find_iter(&code)
                .filter(|hit| !ALLOWED_SHORTCUTS.contains(&hit.as_str()));
            for hit in hits {
                let line = code[..hit.start()].matches('\n').count();
                found.push(Finding {
                    file: file.clone(),
                    line: line + 1,
                    text: lines[line].trim().to_owned(),
                    hit: hit.as_str().to_owned(),
                    rule,
                });
            }
        }
    }
    found.sort_by_key(|finding| (finding.file.clone(), finding.line));
    found
}

fn excepted(finding: &Finding) -> bool {
    EXCEPTIONS.iter().any(|&(file, snippet, _)| {
        finding.file == file && finding.text.contains(snippet) && snippet.contains(&finding.hit)
    })
}

#[test]
fn la_ui_no_escribe_tamanos_ni_colores_fuera_del_tema() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let found = findings(root);
    let loose: Vec<String> = found
        .iter()
        .filter(|finding| !excepted(finding))
        .map(|finding| {
            format!(
                "{}:{}: {} — `{}`",
                finding.file, finding.line, finding.rule, finding.text
            )
        })
        .collect();
    assert!(
        loose.is_empty(),
        "valores sueltos en la UI: muévelos a un token del tema (docs/design.md) \
         o, si de verdad no es estilo, agrégalos a EXCEPTIONS con su razón:\n{}",
        loose.join("\n")
    );
    let stale: Vec<&str> = EXCEPTIONS
        .iter()
        .filter(|&&(file, snippet, _)| {
            !found
                .iter()
                .any(|finding| finding.file == file && finding.text.contains(snippet))
        })
        .map(|&(_, snippet, _)| snippet)
        .collect();
    assert!(
        stale.is_empty(),
        "excepciones que ya no hacen falta: {stale:?}"
    );
}

#[test]
fn cada_excepcion_dice_por_que() {
    for (file, snippet, reason) in EXCEPTIONS {
        assert!(reason.len() > 20, "{file}: `{snippet}` necesita una razón");
    }
}

/// Las reglas atrapan lo que dicen atrapar, y no lo que está permitido.
#[test]
fn las_reglas_atrapan_lo_suelto() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    let loose = [
        ".px(px(10.))",
        ".mt(px(0.5))",
        "let c = Hsla { h: 0., s: 0., l: 0., a: 1. };",
        ".bg(rgb(0x112233))",
        ".border_color(gpui_kit::black())",
        ".rounded_md()",
        ".p_2()",
        ".shadow_lg()",
        "font_weight: Some(FontWeight::BOLD),",
        ".h(size * 0.75)",
        ".w(size * 2)",
        ".max_w(relative(0.5))",
        ".text_color(theme.foreground.opacity(0.5))",
    ];
    let allowed = [
        ".w_full().min_w_0()",
        ".px(theme.layout.panel_padding_x)",
        ".w(relative(1.))",
        "let width = (total - used).max(px(0.));",
        "fn ink(run: &Run) -> Hsla {",
        "// .p_2() en un comentario",
    ];
    let mut text = String::from("fn view() {\n");
    for line in loose.iter().chain(&allowed) {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("}\n#[cfg(test)]\nmod tests { const X: f32 = px(1.); }\n");
    fs::write(src.join("view.rs"), text).unwrap();
    let found: Vec<String> = findings(dir.path())
        .into_iter()
        .map(|finding| finding.text)
        .collect();
    assert_eq!(found, loose);
}

/// Una excepción cubre su valor, no la línea entera: otro valor suelto en
/// la misma línea se reporta igual.
#[test]
fn una_excepcion_no_tapa_otro_valor_de_la_misma_linea() {
    let dir = tempfile::tempdir().unwrap();
    let view = dir.path().join("src/picker/view");
    fs::create_dir_all(&view).unwrap();
    fs::write(
        view.join("render.rs"),
        ".max_w(relative(0.9)).px(px(10.))\n",
    )
    .unwrap();
    let loose: Vec<String> = findings(dir.path())
        .into_iter()
        .filter(|finding| !excepted(finding))
        .map(|finding| finding.hit)
        .collect();
    assert_eq!(loose, ["px(1"]);
}

/// Un valor partido en varias líneas, o debajo de un `#[cfg(test)]` que no
/// abre el módulo de tests, también se ve.
#[test]
fn la_guarda_lee_el_archivo_entero() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    let text = "fn a() {\n    div().px(px(\n        12.,\n    ))\n}\n\
                #[cfg(test)]\nconst X: u8 = 1;\n\
                fn b() { div().bg(rgb(0x112233)) }\n\
                #[cfg(test)]\n#[path = \"x_tests.rs\"]\nmod tests;\n\
                fn c() { px(9.) }\n";
    fs::write(src.join("view.rs"), text).unwrap();
    let found: Vec<(usize, String)> = findings(dir.path())
        .into_iter()
        .map(|finding| (finding.line, finding.hit))
        .collect();
    assert_eq!(
        found,
        [(2, "px(\n        12".to_owned()), (8, "rgb(".to_owned())]
    );
}
