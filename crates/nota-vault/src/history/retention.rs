//! Qué versiones sobreviven, según su edad (06 §3, retención **propuesta**).
//! Las horas y los días son de UTC: no depende de la zona horaria de quien
//! poda, y la misma poda dos veces da lo mismo.

use std::collections::HashSet;

use nota_config::HistoryRetention;

use super::millis;

const HOUR_MS: i64 = 60 * 60 * 1_000;
const DAY_MS: i64 = 24 * HOUR_MS;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    All,
    Hour(i64),
    Day(i64),
    Expired,
}

/// Para cada versión (sus instantes en milisegundos, en el orden en que se
/// guardaron), si se queda. La última se queda siempre.
pub(super) fn keep(saved_at: &[i64], now: i64, retention: &HistoryRetention) -> Vec<bool> {
    let keep_all = millis(retention.keep_all());
    let hourly = millis(retention.hourly());
    let daily = retention.daily().map(millis);
    let slot = |at: i64| {
        let age = now.saturating_sub(at);
        if age <= keep_all {
            Slot::All
        } else if age <= hourly {
            Slot::Hour(at.div_euclid(HOUR_MS))
        } else if daily.is_none_or(|daily| age <= daily) {
            Slot::Day(at.div_euclid(DAY_MS))
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

    const NOW: i64 = 400 * DAY_MS;

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
        let kept = keep(&saved, NOW, &HistoryRetention::default());
        assert_eq!(kept, [false, true, true, true, true]);
    }

    #[test]
    fn una_por_dia_despues_de_30_dias_y_la_ultima_siempre() {
        let day = |n: i64| NOW.div_euclid(DAY_MS) * DAY_MS - n * DAY_MS;
        let saved = [day(100) + 1_000, day(100) + 2_000, day(50) + 1_000, day(40)];
        let kept = keep(&saved, NOW, &HistoryRetention::default());
        assert_eq!(kept, [false, true, true, true]);

        let limited = HistoryRetention {
            daily_days: Some(45),
            ..HistoryRetention::default()
        };
        assert_eq!(keep(&saved, NOW, &limited), [false, false, false, true]);
        assert_eq!(keep(&saved[..1], NOW, &limited), [true]);
    }

    #[test]
    fn podar_dos_veces_da_lo_mismo() {
        let saved: Vec<i64> = (0..2_000).map(|n| ago(n * 37 * 60_000)).rev().collect();
        let retention = HistoryRetention::default();
        let kept = keep(&saved, NOW, &retention);
        let survivors: Vec<i64> = saved
            .iter()
            .zip(&kept)
            .filter(|(_, k)| **k)
            .map(|(at, _)| *at)
            .collect();
        assert!(survivors.len() < saved.len());
        assert!(keep(&survivors, NOW, &retention).iter().all(|k| *k));
    }
}
