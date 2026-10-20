//! Lo que comparten los tests del historial.

use std::time::{Duration, SystemTime};

use metalmind_vault::{History, Retention, VaultPath};

pub const HOUR: Duration = Duration::from_hours(1);
pub const DAY: Duration = Duration::from_hours(24);

/// La de 06 §3, la que trae `config.yaml` por defecto.
pub const RETENTION: Retention = Retention {
    keep_all: DAY,
    hourly: DAY.saturating_mul(30),
    daily: None,
    deleted: DAY.saturating_mul(90),
};

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
