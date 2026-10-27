//! Lo que el motor le pregunta a la vista para partir y medir las líneas:
//! dónde cae cada carácter con la letra con que se pinta (la de un heading,
//! la negrita, la cursiva). Se guarda por texto y estilo en dos
//! generaciones, como las fórmulas: el motor mide las mismas líneas en cada
//! tecla, y así la caché no crece sin tope mientras se escribe.

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use gpui_kit::{Hsla, TextRun, WindowTextSystem};
use metalmind_syntax::{Advances, LineStyle, ScreenLine, ScreenLines};

use super::markdown::runs;
use super::text::Faces;

/// Cuántas líneas guarda cada generación.
const GENERATION: usize = 4096;

pub struct TextScreen {
    text_system: Arc<WindowTextSystem>,
    faces: Faces,
    shapes: Mutex<Generations>,
}

/// Lo que decide dónde cae cada carácter: el texto, su letra en cada tramo,
/// las fórmulas que no se parten y el ancho de las filas.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    size: u32,
    /// `(bytes, peso, cursiva)` de cada tramo.
    fonts: Vec<(usize, u32, bool)>,
    formulas: Vec<Range<usize>>,
    wrap: Option<u32>,
}

#[derive(Default)]
struct Generations {
    current: HashMap<Key, Arc<ScreenLine>>,
    previous: HashMap<Key, Arc<ScreenLine>>,
}

impl TextScreen {
    pub fn new(text_system: Arc<WindowTextSystem>, faces: Faces) -> Self {
        Self {
            text_system,
            faces,
            shapes: Mutex::default(),
        }
    }

    pub fn faces(&self) -> &Faces {
        &self.faces
    }

    fn shape(&self, text: &str, style: Option<&LineStyle>, wrap: Option<f32>) -> ScreenLine {
        let (size, weight) = self.faces.line(style.map(|style| style.kind));
        let runs: Vec<TextRun> = runs(text.len(), style, |_| true, &[])
            .into_iter()
            .map(|run| TextRun {
                len: run.len,
                font: self.faces.run(run.style, weight),
                color: Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            })
            .collect();
        let layout = self.text_system.layout_line(text, size, &runs, None);
        let mut starts: Vec<(usize, f32)> = layout
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .map(|glyph| (glyph.index, f32::from(glyph.position.x)))
            .collect();
        starts.sort_by_key(|&(byte, _)| byte);
        starts.dedup_by_key(|&mut (byte, _)| byte);
        let advances = Advances::new(starts, text.len(), f32::from(layout.width));
        ScreenLine::new(text, advances, style, wrap)
    }

    fn key(&self, text: &str, style: Option<&LineStyle>, wrap: Option<f32>) -> Key {
        let (size, weight) = self.faces.line(style.map(|style| style.kind));
        let fonts = runs(text.len(), style, |_| true, &[])
            .into_iter()
            .map(|run| {
                let font = self.faces.run(run.style, weight);
                let italic = font.style != gpui_kit::FontStyle::Normal;
                (run.len, font.weight.0.to_bits(), italic)
            })
            .collect();
        let formulas = style
            .map(|style| style.math.iter().map(|spot| spot.range.clone()).collect())
            .unwrap_or_default();
        Key {
            text: text.to_owned(),
            size: f32::from(size).to_bits(),
            fonts,
            formulas,
            wrap: wrap.map(f32::to_bits),
        }
    }
}

impl ScreenLines for TextScreen {
    fn line(&self, text: &str, style: Option<&LineStyle>, wrap: Option<f32>) -> Arc<ScreenLine> {
        let key = self.key(text, style, wrap);
        let mut shapes = self.shapes.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(found) = shapes.current.get(&key) {
            return found.clone();
        }
        let found = shapes
            .previous
            .remove(&key)
            .unwrap_or_else(|| Arc::new(self.shape(text, style, wrap)));
        if shapes.current.len() >= GENERATION {
            shapes.previous = std::mem::take(&mut shapes.current);
        }
        shapes.current.insert(key, found.clone());
        found
    }
}

impl fmt::Debug for TextScreen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextScreen")
            .field("faces", &self.faces)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{TestAppContext, font, px};
    use metalmind_config::Config;
    use metalmind_syntax::{Styles, TextLines};

    use super::*;
    use crate::theme::Theme;

    /// Las filas de cada línea de `text` en filas de `width` píxeles, con la
    /// letra del tema de fábrica.
    fn rows(text: &str, width: Option<f32>) -> Vec<Vec<String>> {
        let mut cx = TestAppContext::single();
        let window = cx.add_empty_window();
        window.update(|window, _| {
            let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
            let faces = Faces::new(font("Mono"), px(15.), &theme);
            let screen = TextScreen::new(window.text_system().clone(), faces);
            let lines = TextLines::new(text);
            let count = text.split('\n').count();
            let mut styles = Styles::new(count);
            styles.ensure(usize::MAX, &lines);
            text.split('\n')
                .enumerate()
                .map(|(index, line)| {
                    let shown = screen.line(line, styles.line(index), width);
                    (0..shown.rows())
                        .map(|row| line[shown.row(row)].to_owned())
                        .collect()
                })
                .collect()
        })
    }

    #[test]
    fn parte_con_la_letra_con_que_se_pinta() {
        let prose = "una frase que se repite para que no quepa en una fila ".repeat(4);
        let text = format!("{prose}\n# {prose}");
        let partidas = rows(&text, Some(300.));
        let (body, heading) = (&partidas[0], &partidas[1]);
        assert!(body.len() > 1, "{body:?}");
        assert!(
            heading.len() > body.len(),
            "la letra de un heading es más grande: {heading:?}"
        );
        assert!(
            body.iter()
                .all(|row| row.ends_with(' ') || row == body.last().unwrap()),
            "cada fila termina tras un espacio: {body:?}"
        );
        assert_eq!(body.concat(), prose);
        assert_eq!(rows(&prose, None), [[prose.clone()]], "sin wrap, una fila");
    }

    #[test]
    fn la_misma_linea_se_mide_una_vez() {
        let mut cx = TestAppContext::single();
        let window = cx.add_empty_window();
        window.update(|window, _| {
            let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
            let faces = Faces::new(font("Mono"), px(15.), &theme);
            let screen = TextScreen::new(window.text_system().clone(), faces);
            let first = screen.line("hola mundo", None, Some(40.));
            let again = screen.line("hola mundo", None, Some(40.));
            assert!(Arc::ptr_eq(&first, &again));
            let wider = screen.line("hola mundo", None, Some(400.));
            assert!(!Arc::ptr_eq(&first, &wider), "otro ancho, otras filas");
        });
    }
}
