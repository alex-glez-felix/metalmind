//! Edición consciente de markdown sobre el motor: continuar listas, alternar
//! checkboxes, ir entre headings y la sección como text object. Lo que es
//! un heading o un bloque de código lo decide el estilo de
//! `metalmind-syntax`, el mismo con que el editor pinta la nota.

mod checkbox;
mod heading;
mod list;
mod newline;
mod section;

use std::cell::{OnceCell, RefCell};

use metalmind_buffer::Buffer;
use metalmind_syntax::{LineKind, Styles};

use crate::layout::{BufferLines, Layout};

pub(crate) use checkbox::toggle_checkboxes;
pub(crate) use heading::{next_heading, previous_heading};
pub(crate) use list::ListItem;
pub(crate) use newline::{After, Newline, opened_item, renumber_items};
pub(crate) use section::{section, select_section};

/// Qué partes de la edición de markdown están encendidas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Markdown {
    /// `Enter` en Insert, `o` y `O` sobre un ítem de lista lo continúan (y
    /// renumeran los ítems numerados); `Enter` en un ítem vacío la termina.
    pub continue_lists: bool,
    /// `>>` y `<<` sobre un ítem de lista mueven también sus hijos.
    pub indent_list_children: bool,
}

impl Default for Markdown {
    fn default() -> Self {
        Self {
            continue_lists: true,
            indent_list_children: true,
        }
    }
}

/// Un heading, por su línea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Heading {
    line: usize,
    level: u8,
}

/// Lo que el motor necesita saber de la estructura de la nota: lo mismo que
/// ve el editor al pintarla (`metalmind_syntax::Styles`).
#[derive(Debug)]
pub(crate) struct Outline {
    headings: Vec<Heading>,
    /// Si cada línea es de un bloque de código cercado, cercas incluidas.
    code: Vec<bool>,
}

impl Outline {
    fn of(styles: &Styles) -> Self {
        let mut headings = Vec::new();
        let code = (0..styles.line_count())
            .map(|line| match styles.line(line).map(|style| style.kind) {
                Some(LineKind::Heading(level)) => {
                    headings.push(Heading { line, level });
                    false
                }
                Some(LineKind::Fence(_)) => true,
                _ => false,
            })
            .collect();
        Self { headings, code }
    }

    pub(crate) fn in_code(&self, line: usize) -> bool {
        self.code.get(line).copied().unwrap_or(false)
    }
}

/// El [`Outline`], calculado la primera vez que se pide. Con el [`Layout`]
/// del documento usa su estilo, que ya está calculado para lo que se ve y
/// se rehace solo en lo editado; sin él (un motor sin UI), lo calcula de
/// cero.
pub(crate) struct LazyOutline<'a> {
    layout: RefCell<Option<&'a mut Layout>>,
    outline: OnceCell<Outline>,
}

impl<'a> LazyOutline<'a> {
    /// `layout` ya siguió lo editado en el buffer (`Layout::sync`): lo hace
    /// `Vim::run` antes de cada acción, y quien edita a mitad de una antes
    /// de pedir otro.
    pub(crate) fn new(layout: Option<&'a mut Layout>) -> Self {
        Self {
            layout: RefCell::new(layout),
            outline: OnceCell::new(),
        }
    }

    pub(crate) fn get(&self, buffer: &Buffer) -> &Outline {
        self.outline.get_or_init(|| {
            let last = buffer.last_line();
            if let Some(layout) = self.layout.borrow_mut().as_deref_mut() {
                layout.ensure(buffer, last);
                return Outline::of(layout.styles());
            }
            let mut styles = Styles::new(buffer.line_count());
            styles.ensure(last, &BufferLines(buffer));
            Outline::of(&styles)
        })
    }
}
