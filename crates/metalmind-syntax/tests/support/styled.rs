//! Una nota con sus estilos ya calculados, para medir lo que cuesta una
//! tecla. Compartido por el benchmark `style` y `tests/complexity.rs`.

use std::borrow::Cow;

use metalmind_syntax::{Lines, Styles};

/// Las filas que pinta una ventana: lo que se pide tras cada tecla.
pub const WINDOW: usize = 60;

#[derive(Debug, Clone)]
pub struct NoteLines(Vec<String>);

impl NoteLines {
    pub fn new(text: &str) -> Self {
        Self(text.split('\n').map(str::to_owned).collect())
    }
}

impl Lines for NoteLines {
    fn line_count(&self) -> usize {
        self.0.len()
    }

    fn line(&self, index: usize) -> Cow<'_, str> {
        Cow::Borrowed(self.0.get(index).map_or("", String::as_str))
    }
}

#[derive(Debug, Clone)]
pub struct StyledNote {
    pub lines: NoteLines,
    pub styles: Styles,
}

impl StyledNote {
    pub fn new(text: &str) -> Self {
        let lines = NoteLines::new(text);
        let mut styles = Styles::new(lines.line_count());
        styles.ensure(usize::MAX, &lines);
        Self { lines, styles }
    }

    /// Una letra al final de la línea del medio y la ventana que la muestra.
    pub fn type_in_the_middle(&mut self) {
        let line = self.lines.0.len() / 2;
        self.lines.0[line].push('x');
        self.styles.edit(line, 1, 1);
        self.styles.ensure(line + WINDOW, &self.lines);
    }

    /// Una cerca que no cierra en la línea 10: todo lo de abajo pasa a ser
    /// código, pero solo se pinta la ventana.
    pub fn open_a_fence_at_the_top(&mut self) {
        self.lines.0.insert(10, "```".to_owned());
        self.styles.edit(10, 0, 1);
        self.styles.ensure(WINDOW, &self.lines);
    }
}
