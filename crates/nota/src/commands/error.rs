//! Los errores de los subcomandos, con su código de salida y su forma en JSON.

use std::io::{self, Write as _};
use std::path::PathBuf;

use nota_vault::{CaptureError, VaultError};

use crate::gui::StartError;
use crate::target::TargetError;

/// La clase de un error: decide el código de salida y el `kind` del JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Algo falló al leer o escribir, o no se pudo abrir la ventana.
    Io,
    /// Lo pedido no tiene sentido: argumentos, texto o consulta inválidos.
    Usage,
    /// No hay vault o no se puede abrir.
    Vault,
}

impl ErrorKind {
    /// 2 es también el de clap para los errores de argumentos.
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Io => 1,
            Self::Usage => 2,
            Self::Vault => 3,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Io => "io",
            Self::Usage => "usage",
            Self::Vault => "vault",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error(
        "no hay vault: usa --vault <carpeta>, define NOTA_VAULT, abre uno con la app o pon `vault:` en config.yaml"
    )]
    NoVault,
    #[error(transparent)]
    Target(TargetError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("{} no es una nota del vault", .0.display())]
    NotANote(PathBuf),
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error("no se pudo leer stdin: {0}")]
    Stdin(io::Error),
    #[error("no se pudo escribir la salida: {0}")]
    Output(io::Error),
    #[error(transparent)]
    Start(#[from] StartError),
}

impl CliError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Usage(_)
            | Self::NotANote(_)
            | Self::Capture(
                CaptureError::Empty
                | CaptureError::MultilineTitle
                | CaptureError::InvalidTag(_)
                | CaptureError::FrontmatterWithTags,
            ) => ErrorKind::Usage,
            Self::Stdin(error) if error.kind() == io::ErrorKind::InvalidData => ErrorKind::Usage,
            Self::NoVault | Self::Target(_) | Self::Vault(_) => ErrorKind::Vault,
            Self::Capture(_) | Self::Stdin(_) | Self::Output(_) | Self::Start(_) => ErrorKind::Io,
        }
    }

    /// Lo escribe en stderr: en JSON si se pidió, como `nota: …` si no.
    pub fn report(&self, json: bool) {
        let mut stderr = io::stderr().lock();
        // Si ni stderr acepta el error, no queda dónde decirlo; el código de salida sigue.
        let _ = if json {
            let error = serde_json::json!({
                "error": { "kind": self.kind().as_str(), "message": self.to_string() }
            });
            writeln!(stderr, "{error}")
        } else {
            writeln!(stderr, "nota: {self}")
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_clase_tiene_su_codigo() {
        let codes = [ErrorKind::Io, ErrorKind::Usage, ErrorKind::Vault].map(ErrorKind::exit_code);
        assert_eq!(codes, [1, 2, 3]);
    }

    #[test]
    fn un_stdin_que_no_es_texto_es_error_de_uso() {
        let invalid = CliError::Stdin(io::Error::from(io::ErrorKind::InvalidData));
        assert_eq!(invalid.kind(), ErrorKind::Usage);
        let broken = CliError::Stdin(io::Error::from(io::ErrorKind::BrokenPipe));
        assert_eq!(broken.kind(), ErrorKind::Io);
    }
}
