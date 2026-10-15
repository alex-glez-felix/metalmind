//! Posición que se mueve carácter a carácter como el `pos_T` de vim: puede
//! quedar sobre el fin de línea, que hace el papel del NUL de vim.

use nota_buffer::{Buffer, ByteOffset};

use crate::lines::is_blank;

/// Resultado de un paso, con el mismo significado que `inc()`/`dec()` de vim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Se movió dentro de la línea y no quedó en su fin.
    Moved,
    /// Se movió y quedó en el fin de la línea.
    LineEnd,
    /// Cruzó a otra línea.
    OtherLine,
    /// No se pudo mover: inicio o fin del texto.
    Edge,
}

impl Step {
    pub fn left_line_body(self) -> bool {
        matches!(self, Self::LineEnd | Self::OtherLine)
    }
}

/// Clase de carácter para las motions de palabra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Blank,
    Punctuation,
    Word,
}

impl Class {
    fn of(c: Option<char>, big: bool) -> Self {
        match c {
            None | Some('\n') => Self::Blank,
            Some(c) if is_blank(c) || c == '\u{a0}' => Self::Blank,
            Some(_) if big => Self::Punctuation,
            Some(c) if c == '_' || c.is_alphanumeric() => Self::Word,
            Some(_) => Self::Punctuation,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Cursor<'a> {
    buffer: &'a Buffer,
    pub offset: ByteOffset,
}

impl<'a> Cursor<'a> {
    pub fn new(buffer: &'a Buffer, offset: ByteOffset) -> Self {
        Self { buffer, offset }
    }

    pub fn line(&self) -> usize {
        self.buffer.line_of(self.offset)
    }

    pub fn last_line(&self) -> usize {
        self.buffer.last_line()
    }

    pub fn at_line_start(&self) -> bool {
        self.offset == self.buffer.line_start(self.line())
    }

    pub fn at_line_end(&self) -> bool {
        matches!(self.buffer.char_at(self.offset), None | Some('\n'))
    }

    pub fn on_empty_line(&self) -> bool {
        self.at_line_start() && self.at_line_end()
    }

    pub fn class(&self, big: bool) -> Class {
        Class::of(self.buffer.char_at(self.offset), big)
    }

    pub fn current(&self) -> Option<char> {
        self.buffer.char_at(self.offset).filter(|&c| c != '\n')
    }

    pub fn inc(&mut self) -> Step {
        if !self.at_line_end() {
            self.offset = self.buffer.next_boundary(self.offset);
            return if self.at_line_end() {
                Step::LineEnd
            } else {
                Step::Moved
            };
        }
        if self.offset < self.buffer.len() {
            self.offset = self.offset + 1;
            Step::OtherLine
        } else {
            Step::Edge
        }
    }

    pub fn dec(&mut self) -> Step {
        if !self.at_line_start() {
            self.offset = self.buffer.prev_boundary(self.offset);
            Step::Moved
        } else if self.offset > ByteOffset::ZERO {
            self.offset = self.offset.saturating_sub(1);
            Step::OtherLine
        } else {
            Step::Edge
        }
    }

    /// Como `inc`, pero sin detenerse en el fin de una línea con texto.
    pub fn incl(&mut self) -> Step {
        let step = self.inc();
        if step.left_line_body() && !self.at_line_start() {
            self.inc()
        } else {
            step
        }
    }

    /// Como `dec`, pero sin detenerse en el fin de una línea con texto.
    pub fn decl(&mut self) -> Step {
        let step = self.dec();
        if step == Step::OtherLine && !self.at_line_start() {
            self.dec()
        } else {
            step
        }
    }

    /// Un carácter a la izquierda sin salir de la línea.
    pub fn one_left(&mut self) -> bool {
        if self.at_line_start() {
            return false;
        }
        self.offset = self.buffer.prev_boundary(self.offset);
        true
    }

    /// Avanza mientras la clase sea `class`; `true` si chocó con un borde.
    pub fn skip_class(&mut self, class: Class, big: bool, forward: bool) -> bool {
        while self.class(big) == class {
            let step = if forward { self.inc() } else { self.dec() };
            if step == Step::Edge {
                return true;
            }
        }
        false
    }
}
