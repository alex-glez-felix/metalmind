//! La pregunta por el nombre de una vista guardada, al guardarla o al
//! renombrarla, como un picker de una sola fila: el nombre con su consulta,
//! o por qué todavía no se puede.

use std::ops::Range;

use gpui_kit::{Context, SharedString};
use metalmind_config::SavedView;

use crate::picker::{
    Chosen, InputLine, LineEdit, Open, Overflow, PickerDelegate, PickerRow, PickerView,
};

/// Para qué se pide el nombre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewNamePurpose {
    /// Una vista nueva con esta consulta.
    Save { query: String },
    /// Otro nombre para la vista `from`.
    Rename { from: String, query: String },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ViewNameProblem {
    #[error("Escribe el nombre de la vista; enter la guarda")]
    Empty,
    #[error("ya hay una vista «{0}»")]
    Exists(String),
}

pub struct ViewNameQuestion {
    line: InputLine,
    purpose: ViewNamePurpose,
    /// Los nombres de las otras vistas, que no se pueden repetir.
    taken: Vec<String>,
}

impl ViewNameQuestion {
    /// Al renombrar, el nombre de ahora ya viene escrito.
    #[must_use]
    pub fn new(purpose: ViewNamePurpose, views: &[SavedView]) -> Self {
        let mut line = InputLine::default();
        let from = match &purpose {
            ViewNamePurpose::Rename { from, .. } => {
                line.apply(LineEdit::Insert(from));
                Some(from.as_str())
            }
            ViewNamePurpose::Save { .. } => None,
        };
        let taken = views
            .iter()
            .filter(|view| Some(view.name.as_str()) != from)
            .map(|view| view.name.clone())
            .collect();
        Self {
            line,
            purpose,
            taken,
        }
    }

    #[must_use]
    pub fn purpose(&self) -> &ViewNamePurpose {
        &self.purpose
    }

    /// El nombre escrito, sin espacios a los lados.
    ///
    /// # Errors
    /// Si está vacío o ya lo tiene otra vista, sin distinguir mayúsculas:
    /// `GS` y `gs` en el picker serían la misma.
    pub fn answer(&self) -> Result<&str, ViewNameProblem> {
        let name = self.line.text().trim();
        if name.is_empty() {
            return Err(ViewNameProblem::Empty);
        }
        if let Some(taken) = self
            .taken
            .iter()
            .find(|taken| taken.to_lowercase() == name.to_lowercase())
        {
            return Err(ViewNameProblem::Exists(taken.clone()));
        }
        Ok(name)
    }

    fn query(&self) -> &str {
        match &self.purpose {
            ViewNamePurpose::Save { query } | ViewNamePurpose::Rename { query, .. } => query,
        }
    }
}

impl PickerDelegate for ViewNameQuestion {
    fn line(&self) -> &InputLine {
        &self.line
    }

    fn warnings(&self) -> Vec<Range<usize>> {
        match self.answer() {
            Err(ViewNameProblem::Exists(_)) => std::iter::once(0..self.line.text().len()).collect(),
            Ok(_) | Err(ViewNameProblem::Empty) => Vec::new(),
        }
    }

    fn len(&self) -> usize {
        usize::from(self.answer().is_ok())
    }

    fn selected(&self) -> usize {
        0
    }

    fn select(&mut self, _: usize) {}

    fn select_next(&mut self) {}

    fn select_previous(&mut self) {}

    fn edit(&mut self, edit: LineEdit<'_>, _: &mut Context<PickerView<Self>>) {
        self.line.apply(edit);
    }

    fn choose(&mut self, open: Open, _: &mut Context<PickerView<Self>>) -> Option<Chosen> {
        self.answer().ok().map(|_| Chosen { item: 0, open })
    }

    fn row(&mut self, _: usize) -> PickerRow {
        PickerRow {
            text: self.answer().unwrap_or_default().to_owned(),
            highlights: Vec::new(),
            overflow: Overflow::Around,
            detail: Some(self.query().to_owned()),
        }
    }

    fn empty(&self) -> SharedString {
        self.answer()
            .err()
            .map(|problem| problem.to_string().into())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn views() -> Vec<SavedView> {
        ["gs", "Para hoy"]
            .map(|name| SavedView {
                name: name.into(),
                query: "#x".into(),
            })
            .to_vec()
    }

    #[test]
    fn un_nombre_nuevo_no_repite_otro() {
        let mut question = ViewNameQuestion::new(
            ViewNamePurpose::Save {
                query: "#rust".into(),
            },
            &views(),
        );
        assert_eq!(question.answer(), Err(ViewNameProblem::Empty));
        question.line.apply(LineEdit::Insert("  para HOY "));
        assert_eq!(
            question.answer(),
            Err(ViewNameProblem::Exists("Para hoy".into()))
        );
        assert_eq!(
            question.warnings(),
            std::iter::once(0..11).collect::<Vec<_>>()
        );
        question.line.apply(LineEdit::Insert("2"));
        assert_eq!(question.answer(), Ok("para HOY 2"));
    }

    #[test]
    fn al_renombrar_viene_escrito_y_puede_cambiar_solo_mayusculas() {
        let mut question = ViewNameQuestion::new(
            ViewNamePurpose::Rename {
                from: "gs".into(),
                query: "#x".into(),
            },
            &views(),
        );
        assert_eq!(question.answer(), Ok("gs"));
        question.line.apply(LineEdit::DeleteToStart);
        question.line.apply(LineEdit::Insert("GS"));
        assert_eq!(question.answer(), Ok("GS"));
    }
}
