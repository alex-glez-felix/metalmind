//! De lo que se escribe como nombre de una nota nueva a su ruta y su título.

use metalmind_syntax::note_name;
use metalmind_vault::{InvalidPath, VaultPath};

/// Dónde va una nota nueva y qué llena `{{title}}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePath {
    pub path: VaultPath,
    /// El último componente, sin `.md`.
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("falta el nombre de la nota")]
    Empty,
    #[error(transparent)]
    Path(#[from] InvalidPath),
}

impl NotePath {
    /// `name` relativo a la raíz del vault, con o sin `.md` y con `/` para
    /// subcarpetas, como al crear una nota en el panel lateral.
    ///
    /// # Errors
    /// Si está vacío o no es una ruta del vault.
    pub fn parse(name: &str) -> Result<Self, NameError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(NameError::Empty);
        }
        let file = if note_name::is_note(name) {
            name.to_owned()
        } else {
            note_name::with_extension(name)
        };
        Ok(Self::from_file(VaultPath::new(&file)?))
    }

    pub(crate) fn from_file(path: VaultPath) -> Self {
        let file = path.as_str();
        let stem = note_name::strip_extension(file).unwrap_or(file);
        let title = stem.rsplit('/').next().unwrap_or(stem).to_owned();
        Self { path, title }
    }
}

#[cfg(test)]
mod tests {
    use metalmind_vault::InvalidReason;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::plain("Plan", "Plan.md", "Plan")]
    #[case::trimmed("  Plan del lunes ", "Plan del lunes.md", "Plan del lunes")]
    #[case::with_extension("plan.md", "plan.md", "plan")]
    #[case::upper_extension("Plan.MD", "Plan.MD", "Plan")]
    #[case::folder("reuniones/2026/Plan", "reuniones/2026/Plan.md", "Plan")]
    #[case::dotted("v1.2", "v1.2.md", "v1.2")]
    fn names(#[case] name: &str, #[case] path: &str, #[case] title: &str) {
        let note = NotePath::parse(name).unwrap();
        assert_eq!((note.path.as_str(), note.title.as_str()), (path, title));
    }

    #[rstest]
    #[case::empty("")]
    #[case::blank("   ")]
    fn empty(#[case] name: &str) {
        assert_eq!(NotePath::parse(name), Err(NameError::Empty));
    }

    #[rstest]
    #[case::absolute("/plan", InvalidReason::Absolute)]
    #[case::escapes("../plan", InvalidReason::Relative)]
    #[case::hidden(".plan", InvalidReason::Hidden)]
    fn invalid(#[case] name: &str, #[case] reason: InvalidReason) {
        let Err(NameError::Path(error)) = NotePath::parse(name) else {
            panic!("{name} tenía que fallar");
        };
        assert_eq!(error.reason, reason);
    }
}
