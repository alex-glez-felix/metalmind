//! La fecha de una fila, corta y relativa a hoy: `hoy 23:10`, `ayer`,
//! `24 oct` y, de otro año, `24 oct 2025`.

use std::time::SystemTime;

use jiff::{Timestamp, Zoned};

const MONTHS: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];

/// Cuándo cambió la nota, visto desde `now` y en su zona horaria.
#[must_use]
pub fn when(modified: SystemTime, now: &Zoned) -> String {
    let Ok(at) = Timestamp::try_from(modified) else {
        return String::new();
    };
    let at = at.to_zoned(now.time_zone().clone());
    let (day, today) = (at.date(), now.date());
    if day == today {
        return format!("hoy {:02}:{:02}", at.hour(), at.minute());
    }
    if today.yesterday().is_ok_and(|yesterday| yesterday == day) {
        return "ayer".to_owned();
    }
    let month = MONTHS
        .get(usize::try_from(day.month() - 1).unwrap_or_default())
        .copied()
        .unwrap_or_default();
    if day.year() == today.year() {
        format!("{} {month}", day.day())
    } else {
        format!("{} {month} {}", day.day(), day.year())
    }
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn at(text: &str) -> Zoned {
        let timestamp: Timestamp = text.parse().unwrap();
        timestamp.to_zoned(TimeZone::fixed(jiff::tz::offset(-6)))
    }

    #[rstest]
    #[case::hoy("2026-10-26T22:10:00-06:00", "hoy 22:10")]
    #[case::hoy_temprano("2026-10-26T00:05:00-06:00", "hoy 00:05")]
    #[case::ayer("2026-10-25T23:59:00-06:00", "ayer")]
    #[case::este_anio("2026-01-03T12:00:00-06:00", "3 ene")]
    #[case::otro_anio("2025-12-31T12:00:00-06:00", "31 dic 2025")]
    #[case::en_la_zona_de_ahora("2026-10-27T03:00:00Z", "hoy 21:00")]
    fn relativa_a_hoy(#[case] modified: &str, #[case] expected: &str) {
        let now = at("2026-10-26T23:00:00-06:00");
        let modified: Timestamp = modified.parse().unwrap();
        assert_eq!(when(SystemTime::from(modified), &now), expected);
    }
}
