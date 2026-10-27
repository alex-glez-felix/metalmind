//! Los subcomandos: abren el vault desde el núcleo, sin ventana ni GPUI
//! (06 §7). `open` solo resuelve la nota; la ventana la abre `main`.

mod add;
mod error;
mod open;
mod search;

use std::borrow::Cow;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use metalmind_config::{Paths, Settings};
use metalmind_vault::VaultRoot;
use serde::Serialize;

pub use add::add;
pub use error::CliError;
pub use open::open;
pub use search::search;

use crate::cli::Common;
use crate::target::{Sources, TargetError, default_vault};

/// `config.yaml` sin el tema, y dónde está; la de fábrica si no hay `HOME`.
struct Config {
    paths: Option<Paths>,
    settings: Settings,
}

impl Config {
    fn load() -> Self {
        let paths = Paths::detect().ok();
        let settings = match &paths {
            Some(paths) => {
                let loaded = Settings::load(paths);
                for diagnostic in &loaded.diagnostics {
                    log::warn!("{diagnostic}");
                }
                loaded.value
            }
            None => Settings::default(),
        };
        Self { paths, settings }
    }

    /// El vault de `--vault` o, si falta, el de `metalmind` sin argumentos
    /// ([`default_vault`]).
    fn choose(&self, common: &Common) -> Result<PathBuf, CliError> {
        if let Some(vault) = &common.vault {
            return Ok(vault.clone());
        }
        default_vault(Sources::detect(self.paths.as_ref(), &self.settings)).map_err(|error| {
            match error {
                TargetError::NoVault => CliError::NoVault,
                other => CliError::Target(other),
            }
        })
    }

    /// La raíz del vault elegido, ya comprobada.
    fn vault_root(&self, common: &Common) -> Result<VaultRoot, CliError> {
        Ok(VaultRoot::new(self.choose(common)?)?)
    }
}

/// La raíz como texto para el JSON; una ruta que no es UTF-8 sale con `�`.
fn root_text(root: &Path) -> Cow<'_, str> {
    root.to_string_lossy()
}

/// Escribe `value` como una línea de JSON o, sin `--json`, lo que dé `text`.
fn print<T: Serialize>(
    json: bool,
    value: &T,
    text: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), CliError> {
    let mut stdout = io::stdout().lock();
    let written = if json {
        serde_json::to_writer(&mut stdout, value)
            .map_err(io::Error::from)
            .and_then(|()| writeln!(stdout))
    } else {
        text(&mut stdout)
    };
    match written.and_then(|()| stdout.flush()) {
        Ok(()) => Ok(()),
        // `metalmind search x | head -1`: quien lee ya tiene lo que quería.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(CliError::Output(error)),
    }
}
