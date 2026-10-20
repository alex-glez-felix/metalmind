//! Qué versiones sobreviven, según su edad (06 §3). De cada hora o día se
//! queda la última versión, con las horas y los días de la zona horaria del
//! sistema: "un día" es el día de quien usa nota. En el cambio de horario,
//! la hora que se repite cuenta como una sola.

use std::collections::HashSet;
use std::time::Duration;

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;

use super::millis;

/// Qué versiones de una nota sobreviven con el tiempo. Cada tramo empieza
/// donde acaba el anterior: todas al principio, luego una por hora, luego una
/// por día. La última versión de una nota viva se guarda siempre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retention {
    /// Cuánto tiempo se guardan todas las versiones.
    pub keep_all: Duration,
    /// Hasta qué edad se guarda la última versión de cada hora.
    pub hourly: Duration,
    /// Hasta qué edad se guarda la última versión de cada día; `None` es
    /// para siempre.
    pub daily: Option<Duration>,
    /// Cuánto se guarda la historia de una nota borrada.
    pub deleted: Duration,
}

/// La de 06 §3, la que trae `config.yaml` por defecto.
#[cfg(test)]
pub(crate) const TEST_RETENTION: Retention = Retention {
    keep_all: Duration::from_hours(24),
    hourly: Duration::from_hours(30 * 24),
    daily: None,
    deleted: Duration::from_hours(90 * 24),
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    All,
    Hour(Date, i8),
    Day(Date),
    Expired,
}

/// Para cada versión (sus instantes en milisegundos, en el orden en que se
/// guardaron), si se queda. La última se queda siempre.
pub(super) fn keep(
    saved_at: &[i64],
    now: i64,
    retention: &Retention,
    zone: &TimeZone,
) -> Vec<bool> {
    let keep_all = millis(retention.keep_all);
    let hourly = millis(retention.hourly);
    let daily = retention.daily.map(millis);
    let slot = |at: i64| {
        let age = now.saturating_sub(at);
        // Un instante fuera de lo que `jiff` representa no tiene hora local: se queda.
        let Ok(local) = Timestamp::from_millisecond(at).map(|at| zone.to_datetime(at)) else {
            return Slot::All;
        };
        if age <= keep_all {
            Slot::All
        } else if age <= hourly {
            Slot::Hour(local.date(), local.hour())
        } else if daily.is_none_or(|daily| age <= daily) {
            Slot::Day(local.date())
        } else {
            Slot::Expired
        }
    };
    let mut taken = HashSet::new();
    let mut kept: Vec<bool> = saved_at
        .iter()
        .rev()
        .map(|&at| match slot(at) {
            Slot::All => true,
            Slot::Expired => false,
            // De cada hora o día, la última versión guardada.
            hour_or_day => taken.insert(hour_or_day),
        })
        .collect();
    kept.reverse();
    if let Some(last) = kept.last_mut() {
        *last = true;
    }
    kept
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const HOUR_MS: i64 = 60 * 60 * 1_000;
    const DAY_MS: i64 = 24 * HOUR_MS;
    const NOW: i64 = 400 * DAY_MS;

    fn utc(saved: &[i64], now: i64, retention: &Retention) -> Vec<bool> {
        keep(saved, now, retention, &TimeZone::UTC)
    }

    /// Una zona con horario de verano, como la de Europa central.
    fn central_europe() -> TimeZone {
        TimeZone::posix("CET-1CEST,M3.5.0,M10.5.0/3").unwrap()
    }

    fn at(local: &str) -> i64 {
        local.parse::<Timestamp>().unwrap().as_millisecond()
    }

    fn ago(ms: i64) -> i64 {
        NOW - ms
    }

    #[test]
    fn todas_las_del_ultimo_dia_y_una_por_hora_despues() {
        let saved = [
            ago(3 * DAY_MS + 50 * 60_000),
            ago(3 * DAY_MS + 10 * 60_000),
            ago(3 * DAY_MS),
            ago(2 * HOUR_MS + 1),
            ago(2 * HOUR_MS),
        ];
        let kept = utc(&saved, NOW, &TEST_RETENTION);
        assert_eq!(kept, [false, true, true, true, true]);
    }

    #[test]
    fn una_por_dia_despues_de_30_dias_y_la_ultima_siempre() {
        let day = |n: i64| NOW.div_euclid(DAY_MS) * DAY_MS - n * DAY_MS;
        let saved = [day(100) + 1_000, day(100) + 2_000, day(50) + 1_000, day(40)];
        let kept = utc(&saved, NOW, &TEST_RETENTION);
        assert_eq!(kept, [false, true, true, true]);

        let limited = Retention {
            daily: Some(Duration::from_hours(45 * 24)),
            ..TEST_RETENTION
        };
        assert_eq!(utc(&saved, NOW, &limited), [false, false, false, true]);
        assert_eq!(utc(&saved[..1], NOW, &limited), [true]);
    }

    #[test]
    fn podar_dos_veces_da_lo_mismo() {
        let saved: Vec<i64> = (0..2_000).map(|n| ago(n * 37 * 60_000)).rev().collect();
        let retention = TEST_RETENTION;
        let kept = utc(&saved, NOW, &retention);
        let survivors: Vec<i64> = saved
            .iter()
            .zip(&kept)
            .filter(|(_, k)| **k)
            .map(|(at, _)| *at)
            .collect();
        assert!(survivors.len() < saved.len());
        assert!(utc(&survivors, NOW, &retention).iter().all(|k| *k));
    }

    #[test]
    fn la_hora_que_se_repite_al_cambiar_de_horario_es_una_sola() {
        // El 25 de octubre de 2026 a las 03:00 de verano vuelven a ser las 02:00.
        let saved = [
            at("2026-10-25T01:50:00+02:00"),
            at("2026-10-25T02:10:00+02:00"),
            at("2026-10-25T02:20:00+01:00"),
            at("2026-10-25T03:10:00+01:00"),
        ];
        let now = at("2026-10-28T12:00:00+01:00");
        let retention = TEST_RETENTION;
        assert_eq!(
            keep(&saved, now, &retention, &central_europe()),
            [true, false, true, true]
        );
        // En UTC son cuatro horas distintas.
        assert_eq!(utc(&saved, now, &retention), [true, true, true, true]);
    }

    #[test]
    fn los_dias_son_los_de_la_zona_horaria_local() {
        // 22:00 y 00:30 en Europa central: dos días ahí, uno solo en UTC.
        let saved = [
            at("2026-11-02T22:00:00+01:00"),
            at("2026-11-03T00:30:00+01:00"),
        ];
        let now = at("2027-02-01T12:00:00+01:00");
        let retention = TEST_RETENTION;
        assert_eq!(
            keep(&saved, now, &retention, &central_europe()),
            [true, true]
        );
        assert_eq!(utc(&saved, now, &retention), [false, true]);
    }
}
