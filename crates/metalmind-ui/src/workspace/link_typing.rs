//! El wikilink que se está escribiendo en Insert y cómo aceptar su
//! autocompletado sin salir de la pasada de Insert. Lo dice el estilo de la
//! línea (`LineStyle::typed_wikilink`), así que no se escanea la nota.

use metalmind_vim::{Clipboard, Mode};

use super::{PaneId, Workspace, WorkspaceError};

/// El destino de un wikilink a medio escribir, con el cursor al final.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedLink {
    pub typed: String,
    /// Si un `]]` ya lo cierra.
    pub closed: bool,
}

impl Workspace {
    /// El wikilink que se está escribiendo en `pane`: en Insert, con el
    /// cursor justo después de lo escrito de su destino.
    ///
    /// # Errors
    /// Si `pane` no existe.
    pub fn typed_link(&self, pane: PaneId) -> Result<Option<TypedLink>, WorkspaceError> {
        let view = self.view(pane)?;
        if view.mode != Mode::Insert {
            return Ok(None);
        }
        let buffer = view.buffer();
        let cursor = view.cursor();
        let line = buffer.line_of(cursor);
        let Some(style) = view
            .document
            .state()
            .styles()
            .and_then(|styles| styles.line(line))
        else {
            return Ok(None);
        };
        let start = buffer.line_start(line);
        let text = view.document.line_text(line)?;
        let Some(found) = style
            .typed_wikilink(&text, cursor.get() - start.get())
            .filter(|found| {
                self.can_type_over(&view, start + found.target.start..start + found.target.end)
            })
        else {
            return Ok(None);
        };
        Ok(Some(TypedLink {
            typed: text[found.target].to_owned(),
            closed: found.closed,
        }))
    }

    /// Cambia lo escrito del destino por `target` y, si nada lo cierra,
    /// escribe el `]]`, como si se tecleara (ver [`Workspace::type_over`]).
    /// Sin wikilink a medio escribir, no hace nada.
    ///
    /// # Errors
    /// Si `pane` no existe o una edición no cae en el buffer.
    pub fn complete_link(
        &mut self,
        pane: PaneId,
        target: &str,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), WorkspaceError> {
        let Some(typed) = self.typed_link(pane)? else {
            return Ok(());
        };
        let text = if typed.closed {
            target.to_owned()
        } else {
            format!("{target}]]")
        };
        self.type_over(pane, &typed.typed, &text, clipboard)
    }
}
