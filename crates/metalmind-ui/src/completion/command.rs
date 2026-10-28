//! La lista de `:` fuera de un argumento, sin GPUI: con la línea vacía, lo
//! enviado antes; al escribir un nombre, los comandos que empiezan con él.
//! Aceptar escribe la fila elegida.

use metalmind_vim::TypedCommand;

use super::Selection;

pub struct CommandCompletions {
    typed: TypedCommand,
    selection: Selection,
}

/// Lo que muestra una fila.
#[derive(Debug, PartialEq, Eq)]
pub struct CommandRow<'a> {
    /// Una línea del historial o el nombre de un comando.
    pub text: &'a str,
    /// La acción que corre un comando ajeno.
    pub action: Option<&'static str>,
}

impl CommandCompletions {
    /// La lista de `typed`; si `previous` es la de lo mismo, su fila
    /// elegida sigue elegida.
    #[must_use]
    pub fn open(typed: TypedCommand, previous: Option<Self>) -> Self {
        match previous {
            Some(previous) if previous.typed == typed => previous,
            _ => Self {
                selection: Selection::new(len(&typed)),
                typed,
            },
        }
    }

    #[must_use]
    pub fn row(&self, row: usize) -> Option<CommandRow<'_>> {
        match &self.typed {
            TypedCommand::Empty { history } => history.get(row).map(|line| CommandRow {
                text: line,
                action: None,
            }),
            TypedCommand::Name { names, .. } => names.get(row).map(|name| CommandRow {
                text: name.name,
                action: name.action.map(metalmind_vim::ActionName::as_str),
            }),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        len(&self.typed)
    }

    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// Lo que escribe la fila elegida en lugar de lo escrito: la línea del
    /// historial entera, o el nombre con el espacio de su argumento.
    #[must_use]
    pub fn accept(&self) -> String {
        let row = self.selection.selected();
        match &self.typed {
            TypedCommand::Empty { history } => history.get(row).cloned().unwrap_or_default(),
            TypedCommand::Name { names, .. } => names
                .get(row)
                .map(metalmind_vim::CommandName::completion)
                .unwrap_or_default(),
        }
    }
}

fn len(typed: &TypedCommand) -> usize {
    match typed {
        TypedCommand::Empty { history } => history.len(),
        TypedCommand::Name { names, .. } => names.len(),
    }
}
