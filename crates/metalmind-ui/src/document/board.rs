//! Editar el archivo de una pizarra: siempre con líneas enteras (09 §4.3), así
//! que deshacer, guardar y releer son los del buffer. El texto de una caja se
//! escribe en otro buffer con el motor de vim; lo que cambia ahí pasa al
//! archivo dentro de una sola transacción, que se cierra al salir de la caja.

use metalmind_board::{Board, FormatError, Frame, Shape, line_edits};
use metalmind_buffer::{Buffer, BufferError, ByteOffset, Edit, Selection, Selections};
use metalmind_syntax::Rows;
use metalmind_vim::DocState;

use super::{Document, EOL};

#[derive(Debug, thiserror::Error)]
pub enum BoardEditError {
    /// El archivo no se lee como pizarra: no se toca.
    #[error("No se lee como pizarra: {0}")]
    Format(#[from] FormatError),
    #[error(transparent)]
    Buffer(#[from] BufferError),
}

/// Lo que el motor edita en lugar del archivo: nada de lo que escriba `:` puede
/// romper su JSON (09 §4.2). Fuera de una caja está vacío.
#[derive(Debug)]
pub(super) struct EngineScratch {
    pub(super) buffer: Buffer,
    pub(super) state: DocState,
    /// La caja cuyo texto tiene, mientras se escribe en ella.
    text: Option<OpenText>,
}

#[derive(Debug)]
struct OpenText {
    element: String,
    /// La caja nació con esta edición: si queda vacía, no llegó a existir.
    created: bool,
}

impl EngineScratch {
    pub(super) fn holding(text: &str) -> Self {
        let buffer = Buffer::from(text);
        Self {
            state: DocState::painted(&buffer, Rows::uniform(1)),
            buffer,
            text: None,
        }
    }
}

/// El texto que se está escribiendo en una pizarra, para pintarlo.
#[derive(Clone, Copy, Debug)]
pub struct OpenBoardText<'a> {
    pub element: &'a str,
    pub buffer: &'a Buffer,
}

fn at_start() -> Selections {
    Selections::single(Selection::cursor(ByteOffset::ZERO))
}

impl Document {
    /// El elemento y el buffer de la caja que se está escribiendo, si hay una.
    #[must_use]
    pub fn open_board_text(&self) -> Option<OpenBoardText<'_>> {
        let scratch = self.board_scratch.as_ref()?;
        let open = scratch.text.as_ref()?;
        Some(OpenBoardText {
            element: &open.element,
            buffer: &scratch.buffer,
        })
    }

    /// Cambia la pizarra y pasa al buffer solo las líneas que cambiaron, como
    /// un paso del historial; con una caja abierta, dentro de su paso.
    ///
    /// # Errors
    /// Si el archivo no se lee como pizarra: no se toca nada.
    pub(crate) fn edit_board<T>(
        &mut self,
        change: impl FnOnce(&mut Board) -> T,
    ) -> Result<T, BoardEditError> {
        let old = self.buffer.to_string();
        let mut board = Board::parse(&old)?;
        let before = board.clone();
        let result = change(&mut board);
        if board != before {
            self.write_board(&old, &board)?;
        }
        Ok(result)
    }

    fn write_board(&mut self, old: &str, board: &Board) -> Result<(), BufferError> {
        let written = board.write();
        let new = written.strip_suffix(EOL).unwrap_or(&written);
        let edits = line_edits(old, new)
            .into_iter()
            .map(|edit| {
                let range = ByteOffset::new(edit.range.start)..ByteOffset::new(edit.range.end);
                Edit::replace(range, edit.text)
            })
            .collect();
        let standalone = !self.buffer.has_open_transaction();
        if standalone {
            self.buffer.begin_transaction(at_start())?;
        }
        self.buffer.apply(edits)?;
        if standalone {
            self.buffer.commit_transaction(at_start())?;
        }
        self.forget_buffer_changes();
        Ok(())
    }

    /// El motor no pinta este buffer ni tiene cursores en él: nadie va a leer
    /// lo que el buffer anota para ellos.
    fn forget_buffer_changes(&mut self) {
        self.buffer.take_splices();
        self.buffer.take_line_edits();
    }

    /// Un paso atrás en el historial de la pizarra; `false` si no hay más.
    ///
    /// # Errors
    /// Si hay una caja abierta.
    pub(crate) fn undo_board(&mut self) -> Result<bool, BufferError> {
        let moved = self.buffer.undo()?.is_some();
        self.forget_buffer_changes();
        Ok(moved)
    }

    /// # Errors
    /// Si hay una caja abierta.
    pub(crate) fn redo_board(&mut self) -> Result<bool, BufferError> {
        let moved = self.buffer.redo()?.is_some();
        self.forget_buffer_changes();
        Ok(moved)
    }

    /// Abre el texto de `element` en el motor; `false` si no tiene texto.
    ///
    /// # Errors
    /// Si el archivo no se lee como pizarra o ya hay una caja abierta.
    pub(crate) fn begin_board_text(&mut self, element: &str) -> Result<bool, BoardEditError> {
        let board = Board::parse(&self.buffer.to_string())?;
        let text = if let Some(edge) = board.edge(element) {
            edge.label.as_deref().unwrap_or_default()
        } else if let Some(text) = board.node(element).and_then(|node| node.text()) {
            text
        } else {
            return Ok(false);
        };
        self.buffer.begin_transaction(at_start())?;
        self.load_scratch(text, element, false);
        Ok(true)
    }

    /// Crea un texto suelto en `frame` y lo abre: crearlo y escribirlo son un
    /// solo paso. Da su id.
    ///
    /// # Errors
    /// Si el archivo no se lee como pizarra o ya hay una caja abierta.
    pub(crate) fn begin_new_board_text(&mut self, frame: Frame) -> Result<String, BoardEditError> {
        self.buffer.begin_transaction(at_start())?;
        let added = self.edit_board(|board| board.add_text(frame, Shape::None, ""));
        match added {
            Ok(id) => {
                self.load_scratch("", &id, true);
                Ok(id)
            }
            Err(error) => {
                self.buffer.commit_transaction(at_start())?;
                Err(error)
            }
        }
    }

    fn load_scratch(&mut self, text: &str, element: &str, created: bool) {
        let mut scratch = EngineScratch::holding(text);
        scratch.text = Some(OpenText {
            element: element.to_owned(),
            created,
        });
        self.board_scratch = Some(scratch);
    }

    /// Lo que hay en el buffer de la caja pasa a su línea del archivo.
    ///
    /// # Errors
    /// Si el archivo dejó de leerse como pizarra.
    pub(crate) fn sync_board_text(&mut self) -> Result<(), BoardEditError> {
        let Some(open) = self.open_board_text() else {
            return Ok(());
        };
        let element = open.element.to_owned();
        let text = open.buffer.to_string();
        self.edit_board(|board| {
            board.set_text(&element, &text);
        })
    }

    /// Cierra la caja abierta: todo lo escrito queda como un solo paso. Un
    /// texto suelto vacío no se ve ni se puede elegir, así que no se queda.
    ///
    /// # Errors
    /// Si el buffer está inconsistente, lo que sería un bug.
    pub(crate) fn end_board_text(&mut self) -> Result<(), BoardEditError> {
        let Some(open) = self
            .board_scratch
            .replace(EngineScratch::holding(""))
            .and_then(|scratch| scratch.text)
        else {
            return Ok(());
        };
        let vanishes = |board: &Board| {
            board.node(&open.element).is_some_and(|node| {
                node.text() == Some("")
                    && matches!(
                        node.kind,
                        metalmind_board::NodeKind::Text {
                            shape: Shape::None,
                            ..
                        }
                    )
            })
        };
        let empty = vanishes(&Board::parse(&self.buffer.to_string())?);
        if empty && !open.created {
            self.edit_board(|board| board.remove(std::slice::from_ref(&open.element)))?;
        }
        self.buffer.commit_transaction(at_start())?;
        if empty && open.created {
            self.buffer.undo()?;
        }
        self.forget_buffer_changes();
        self.settle();
        Ok(())
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
