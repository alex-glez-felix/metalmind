//! Edición consciente de markdown sobre el motor: continuar listas, alternar
//! checkboxes, ir entre headings, la sección como text object y alinear
//! tablas. Lo que es
//! un heading o un bloque de código lo decide el estilo de
//! `metalmind-syntax`, el mismo con que el editor pinta la nota.

mod checkbox;
mod heading;
mod list;
mod newline;
mod section;
mod table;

use std::cell::{OnceCell, RefCell};

use metalmind_buffer::Buffer;
use metalmind_syntax::{LineKind, LineStyle, Styles};

use crate::doc::BufferLines;

pub(crate) use checkbox::toggle_checkboxes;
pub(crate) use heading::{next_heading, previous_heading};
pub(crate) use list::{ListItem, indent_of};
pub(crate) use newline::{After, Newline, opened_item, renumber_items};
pub(crate) use section::{section, select_section};
pub(crate) use table::TableFormat;

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
    /// Si cada línea es texto literal y no markdown: de un bloque de código
    /// cercado (cercas incluidas) o del frontmatter.
    literal: Vec<bool>,
}

impl Outline {
    fn of(styles: &Styles) -> Self {
        let mut headings = Vec::new();
        let literal = (0..styles.line_count())
            .map(|line| match styles.line(line).map(|style| style.kind) {
                Some(LineKind::Heading(level)) => {
                    headings.push(Heading { line, level });
                    false
                }
                Some(LineKind::Fence(_) | LineKind::Frontmatter) => true,
                _ => false,
            })
            .collect();
        Self { headings, literal }
    }

    /// Las listas no se editan ahí: un `- a` del frontmatter es YAML.
    pub(crate) fn is_literal(&self, line: usize) -> bool {
        self.literal.get(line).copied().unwrap_or(false)
    }
}

/// El estilo del documento, calculado a pedido: el de una línea o el
/// [`Outline`], la primera vez que se pide. Con el estilo del documento que
/// se pinta ([`crate::DocState`]) usa ese, que ya está calculado para lo
/// que se ve y se rehace solo en lo editado; sin él (un motor sin UI), el
/// outline se calcula de cero y una línea no tiene estilo.
pub(crate) struct LazyStyles<'a> {
    styles: RefCell<Option<&'a mut Styles>>,
    outline: OnceCell<Outline>,
}

impl<'a> LazyStyles<'a> {
    /// `styles` ya siguió lo editado en el buffer (`DocState::sync`): lo
    /// hace `Vim::run` antes de cada acción, y quien edita a mitad de una
    /// antes de pedir otro.
    pub(crate) fn new(styles: Option<&'a mut Styles>) -> Self {
        Self {
            styles: RefCell::new(styles),
            outline: OnceCell::new(),
        }
    }

    /// El estilo de `line`, si el documento se pinta.
    pub(crate) fn line(&self, buffer: &Buffer, line: usize) -> Option<LineStyle> {
        let mut styles = self.styles.borrow_mut();
        let styles = styles.as_deref_mut()?;
        styles.ensure(line, &BufferLines(buffer));
        styles.line(line).cloned()
    }

    pub(crate) fn outline(&self, buffer: &Buffer) -> &Outline {
        self.outline.get_or_init(|| {
            let last = buffer.last_line();
            if let Some(styles) = self.styles.borrow_mut().as_deref_mut() {
                styles.ensure(last, &BufferLines(buffer));
                return Outline::of(styles);
            }
            let mut styles = Styles::new(buffer.line_count());
            styles.ensure(last, &BufferLines(buffer));
            Outline::of(&styles)
        })
    }
}
