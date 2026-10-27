//! Lo que las vistas del panel comparten: moverse por las filas, abrir
//! y cerrar, y qué archivo abre la fila seleccionada. Las acciones del panel
//! (`j`, `l`, `enter`, el clic) van a la vista que se ve.

use metalmind_vault::VaultPath;

use crate::workspace::LinePosition;

pub trait Outline {
    fn selected_row(&self) -> Option<usize>;
    /// Selecciona `row`, o la última si no hay tantas.
    fn select_row(&mut self, row: usize);
    /// `h`: cierra la fila o sube a la que la contiene.
    fn collapse_or_parent(&mut self);
    /// `l`: abre la fila o baja a su primera hija.
    fn expand_or_enter(&mut self);
    /// Abre o cierra la fila seleccionada, si se abre.
    fn toggle_selected(&mut self);
    /// La entrada del vault de la fila seleccionada, si tiene una.
    fn selected_path(&self) -> Option<&VaultPath>;
    /// El archivo que abre la fila seleccionada: no una carpeta ni un tag.
    fn selected_file(&self) -> Option<&VaultPath>;
    /// Dónde abre ese archivo; `None` es donde quedó el cursor la última vez.
    fn selected_position(&self) -> Option<LinePosition> {
        None
    }

    fn select_next(&mut self) {
        let row = self.selected_row().map_or(0, |row| row + 1);
        self.select_row(row);
    }

    fn select_previous(&mut self) {
        let row = self.selected_row().map_or(0, |row| row.saturating_sub(1));
        self.select_row(row);
    }

    fn select_first(&mut self) {
        self.select_row(0);
    }

    fn select_last(&mut self) {
        self.select_row(usize::MAX);
    }
}
