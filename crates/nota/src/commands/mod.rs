//! Los subcomandos: abren el vault desde el núcleo, sin ventana ni GPUI
//! (06 §7). `open` solo resuelve la nota; la ventana la abre `main`.

mod add;
mod error;
mod open;
mod search;

use std::borrow::Cow;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use nota_config::{Paths, Settings};
use nota_vault::{Vault, VaultOptions, VaultRoot};
use serde::Serialize;

pub use add::add;
pub use error::CliError;
pub use open::open;
pub use search::search;

use crate::cli::Common;
use crate::target::{Sources, TargetError, default_vault};

/// El vault de `--vault` o, si falta, el de `nota` sin argumentos
/// ([`default_vault`]). Solo lee `config.yaml`, sin el tema.
fn choose(common: &Common) -> Result<PathBuf, CliError> {
    if let Some(vault) = &common.vault {
        return Ok(vault.clone());
    }
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
    default_vault(Sources::detect(paths.as_ref(), &settings)).map_err(|error| match error {
        TargetError::NoVault => CliError::NoVault,
        other => CliError::Target(other),
    })
}

/// El vault elegido, indexado.
fn open_vault(common: &Common) -> Result<Vault, CliError> {
    Ok(Vault::load(choose(common)?, VaultOptions::default())?)
}

/// Solo la raíz del vault elegido, sin recorrerlo.
fn vault_root(common: &Common) -> Result<VaultRoot, CliError> {
    Ok(VaultRoot::new(choose(common)?)?)
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
        // `nota search x | head -1`: quien lee ya tiene lo que quería.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(CliError::Output(error)),
    }
}
