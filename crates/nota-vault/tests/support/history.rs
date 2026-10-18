//! Lo que comparten los tests del historial.

use std::time::{Duration, SystemTime};

use nota_vault::{History, VaultPath};

pub const HOUR: Duration = Duration::from_hours(1);
pub const DAY: Duration = Duration::from_hours(24);

pub fn path(path: &str) -> VaultPath {
    VaultPath::new(path).unwrap()
}

/// Un instante fijo, alineado al día, para que la retención sea exacta.
pub fn epoch() -> SystemTime {
    SystemTime::UNIX_EPOCH + 20_000 * DAY
}

/// El contenido de cada versión de `note`, de la más vieja a la más nueva.
pub fn contents(history: &History, note: &VaultPath) -> Vec<String> {
    history
        .versions(note)
        .unwrap()
        .iter()
        .map(|version| history.content(note, version.id).unwrap())
        .collect()
}
