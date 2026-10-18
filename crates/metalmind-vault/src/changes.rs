//! Lo que una refactorización hace en el vault, como datos: editar rangos de
//! archivos, crear archivos nuevos y renombrar, en ese orden. Se arma sin
//! tocar el disco para mostrarlo como vista previa, y [`crate::Vault::apply`]
//! lo aplica con deshacer (05 §5). Renombrar notas y renombrar tags usan lo
//! mismo.

use crate::edit::FileEdits;
use crate::path::VaultPath;

/// Un archivo que no existe y se crea con `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFile {
    pub path: VaultPath,
    pub text: String,
}

/// Mover `from` a `to`, sin pisar nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub from: VaultPath,
    pub to: VaultPath,
}

/// Un cambio en varios archivos. Se aplica en orden: primero las ediciones
/// (con las rutas de antes de renombrar), luego los archivos nuevos y al
/// final los renombres.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// En orden de ruta, cada una con sus ediciones en orden.
    pub edits: Vec<FileEdits>,
    pub creates: Vec<NewFile>,
    pub renames: Vec<Rename>,
}

impl Changes {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty() && self.creates.is_empty() && self.renames.is_empty()
    }

    /// Cuántas ediciones de texto hay, para el resumen de la vista previa.
    #[must_use]
    pub fn edit_count(&self) -> usize {
        self.edits.iter().map(|file| file.edits.len()).sum()
    }
}
