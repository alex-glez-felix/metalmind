//! Cuánto historial de versiones se guarda de cada nota (06 §3).

use std::time::Duration;

use serde::Deserialize;

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;

/// Qué versiones de una nota sobreviven con el tiempo. Cada tramo empieza
/// donde acaba el anterior: todas al principio, luego una por hora, luego una
/// por día. La última versión de una nota viva se guarda siempre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct HistoryRetention {
    /// Horas en las que se guardan todas las versiones.
    pub keep_all_hours: u32,
    /// Días en los que se guarda la última versión de cada hora.
    pub hourly_days: u32,
    /// Días en los que se guarda la última versión de cada día; `None` es
    /// para siempre.
    pub daily_days: Option<u32>,
    /// Días que se guarda la historia de una nota borrada.
    pub deleted_days: u32,
}

impl HistoryRetention {
    #[must_use]
    pub fn keep_all(&self) -> Duration {
        Duration::from_secs(u64::from(self.keep_all_hours) * HOUR)
    }

    #[must_use]
    pub fn hourly(&self) -> Duration {
        Duration::from_secs(u64::from(self.hourly_days) * DAY)
    }

    #[must_use]
    pub fn daily(&self) -> Option<Duration> {
        self.daily_days
            .map(|days| Duration::from_secs(u64::from(days) * DAY))
    }

    #[must_use]
    pub fn deleted(&self) -> Duration {
        Duration::from_secs(u64::from(self.deleted_days) * DAY)
    }
}

impl Default for HistoryRetention {
    fn default() -> Self {
        Self {
            keep_all_hours: 24,
            hourly_days: 30,
            daily_days: None,
            deleted_days: 90,
        }
    }
}
