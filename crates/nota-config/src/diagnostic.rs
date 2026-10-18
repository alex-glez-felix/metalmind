//! Errores y avisos de configuración, con archivo, línea y columna.

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Se ignoró algo (p. ej. una clave desconocida) y el resto se aplicó.
    Warning,
    /// No se pudo usar el archivo o el tema; se usan los valores por defecto.
    Error,
}

/// Línea y columna, contadas desde 1; la columna en caracteres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Location {
    /// La posición del byte `offset` de `text`.
    #[must_use]
    pub fn of(text: &str, offset: usize) -> Self {
        let before = text.get(..offset).unwrap_or(text);
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        Self {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: Option<PathBuf>,
    pub location: Option<Location>,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            file: None,
            location: None,
            message: message.into(),
        }
    }

    pub(crate) fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(message)
        }
    }

    pub(crate) fn at(self, location: Option<Location>) -> Self {
        Self { location, ..self }
    }

    #[must_use]
    pub fn in_file(self, file: &Path) -> Self {
        Self {
            file: Some(file.to_owned()),
            ..self
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{}:", file.display())?;
        }
        if let Some(Location { line, column }) = self.location {
            write!(f, "{line}:{column}:")?;
        }
        if self.file.is_some() || self.location.is_some() {
            f.write_str(" ")?;
        }
        let severity = match self.severity {
            Severity::Warning => "aviso",
            Severity::Error => "error",
        };
        write!(f, "{severity}: {}", self.message)
    }
}

/// Un valor que siempre se puede usar, con lo que hubo que reportar para
/// llegar a él.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded<T> {
    pub value: T,
    pub diagnostics: Vec<Diagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linea_y_columna_desde_uno_en_caracteres() {
        let text = "a = 1\nñandú = x\n";
        assert_eq!(Location::of(text, 0), Location { line: 1, column: 1 });
        let x = text.find('x').unwrap();
        assert_eq!(Location::of(text, x), Location { line: 2, column: 9 });
    }

    #[test]
    fn formato_como_el_de_un_compilador() {
        let diagnostic = Diagnostic::error("clave rara")
            .at(Some(Location { line: 3, column: 7 }))
            .in_file(Path::new("config.toml"));
        assert_eq!(diagnostic.to_string(), "config.toml:3:7: error: clave rara");
        assert_eq!(Diagnostic::warning("ojo").to_string(), "aviso: ojo");
    }
}
