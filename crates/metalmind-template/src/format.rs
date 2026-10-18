//! El formato de `{{date:…}}` y de los nombres de la nota del día: los
//! códigos más comunes de moment.js.
//! Una letra que no es código se copia tal cual, como en moment.

use jiff::Zoned;
use jiff::civil::Weekday;

/// Un formato ya leído, listo para aplicarse a cualquier fecha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateFormat(Vec<Piece>);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    #[error("el formato de fecha está vacío")]
    Empty,
    #[error("el `[` del byte {at} del formato no se cierra")]
    UnclosedBracket { at: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Literal(String),
    Code(Code),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Code {
    Year,
    ShortYear,
    MonthName,
    MonthAbbr,
    PaddedMonth,
    Month,
    PaddedDay,
    Day,
    WeekdayName,
    WeekdayAbbr,
    PaddedHour,
    Hour,
    PaddedHour12,
    Hour12,
    PaddedMinute,
    Minute,
    PaddedSecond,
    Second,
    UpperMeridiem,
    LowerMeridiem,
    Offset,
    CompactOffset,
}

/// Los más largos primero: `MM` no debe leerse como dos `M`.
const CODES: [(&str, Code); 22] = [
    ("YYYY", Code::Year),
    ("MMMM", Code::MonthName),
    ("dddd", Code::WeekdayName),
    ("MMM", Code::MonthAbbr),
    ("ddd", Code::WeekdayAbbr),
    ("YY", Code::ShortYear),
    ("MM", Code::PaddedMonth),
    ("DD", Code::PaddedDay),
    ("HH", Code::PaddedHour),
    ("hh", Code::PaddedHour12),
    ("mm", Code::PaddedMinute),
    ("ss", Code::PaddedSecond),
    ("ZZ", Code::CompactOffset),
    ("M", Code::Month),
    ("D", Code::Day),
    ("H", Code::Hour),
    ("h", Code::Hour12),
    ("m", Code::Minute),
    ("s", Code::Second),
    ("A", Code::UpperMeridiem),
    ("a", Code::LowerMeridiem),
    ("Z", Code::Offset),
];

const MONTHS: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

const MONTH_ABBRS: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];

impl DateFormat {
    /// `YYYY-MM-DD`, el de `{{date}}`.
    #[must_use]
    pub fn date() -> Self {
        Self(vec![
            Piece::Code(Code::Year),
            Piece::Literal("-".to_owned()),
            Piece::Code(Code::PaddedMonth),
            Piece::Literal("-".to_owned()),
            Piece::Code(Code::PaddedDay),
        ])
    }

    /// `HH:mm`, el de `{{time}}`.
    #[must_use]
    pub fn time() -> Self {
        Self(vec![
            Piece::Code(Code::PaddedHour),
            Piece::Literal(":".to_owned()),
            Piece::Code(Code::PaddedMinute),
        ])
    }

    /// # Errors
    /// Si está vacío o tiene un `[` sin cerrar.
    pub fn parse(format: &str) -> Result<Self, FormatError> {
        if format.is_empty() {
            return Err(FormatError::Empty);
        }
        let mut pieces = Vec::new();
        let mut at = 0;
        while at < format.len() {
            let rest = &format[at..];
            if let Some(quoted) = rest.strip_prefix('[') {
                let close = quoted
                    .find(']')
                    .ok_or(FormatError::UnclosedBracket { at })?;
                push_literal(&mut pieces, &quoted[..close]);
                at += close + 2;
            } else if let Some(&(code, kind)) =
                CODES.iter().find(|(code, _)| rest.starts_with(code))
            {
                pieces.push(Piece::Code(kind));
                at += code.len();
            } else {
                let len = rest.chars().next().map_or(1, char::len_utf8);
                push_literal(&mut pieces, &rest[..len]);
                at += len;
            }
        }
        Ok(Self(pieces))
    }

    /// La fecha en este formato, en la zona horaria de `now`.
    #[must_use]
    pub fn format(&self, now: &Zoned) -> String {
        let mut out = String::new();
        for piece in &self.0 {
            match piece {
                Piece::Literal(text) => out.push_str(text),
                Piece::Code(code) => out.push_str(&code_text(*code, now)),
            }
        }
        out
    }
}

fn push_literal(pieces: &mut Vec<Piece>, text: &str) {
    if let Some(Piece::Literal(last)) = pieces.last_mut() {
        last.push_str(text);
    } else if !text.is_empty() {
        pieces.push(Piece::Literal(text.to_owned()));
    }
}

fn code_text(code: Code, now: &Zoned) -> String {
    let hour12 = match now.hour() % 12 {
        0 => 12,
        hour => hour,
    };
    let meridiem = if now.hour() < 12 { "am" } else { "pm" };
    match code {
        Code::Year => format!("{:04}", now.year()),
        Code::ShortYear => format!("{:02}", now.year().rem_euclid(100)),
        Code::MonthName => MONTHS[month_index(now)].to_owned(),
        Code::MonthAbbr => MONTH_ABBRS[month_index(now)].to_owned(),
        Code::PaddedMonth => format!("{:02}", now.month()),
        Code::Month => now.month().to_string(),
        Code::PaddedDay => format!("{:02}", now.day()),
        Code::Day => now.day().to_string(),
        Code::WeekdayName => weekday_name(now.weekday()).to_owned(),
        Code::WeekdayAbbr => weekday_abbr(now.weekday()).to_owned(),
        Code::PaddedHour => format!("{:02}", now.hour()),
        Code::Hour => now.hour().to_string(),
        Code::PaddedHour12 => format!("{hour12:02}"),
        Code::Hour12 => hour12.to_string(),
        Code::PaddedMinute => format!("{:02}", now.minute()),
        Code::Minute => now.minute().to_string(),
        Code::PaddedSecond => format!("{:02}", now.second()),
        Code::Second => now.second().to_string(),
        Code::UpperMeridiem => meridiem.to_uppercase(),
        Code::LowerMeridiem => meridiem.to_owned(),
        Code::Offset => offset(now, ":"),
        Code::CompactOffset => offset(now, ""),
    }
}

fn month_index(now: &Zoned) -> usize {
    usize::from(now.month().unsigned_abs()) - 1
}

fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "lunes",
        Weekday::Tuesday => "martes",
        Weekday::Wednesday => "miércoles",
        Weekday::Thursday => "jueves",
        Weekday::Friday => "viernes",
        Weekday::Saturday => "sábado",
        Weekday::Sunday => "domingo",
    }
}

fn weekday_abbr(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "lun",
        Weekday::Tuesday => "mar",
        Weekday::Wednesday => "mié",
        Weekday::Thursday => "jue",
        Weekday::Friday => "vie",
        Weekday::Saturday => "sáb",
        Weekday::Sunday => "dom",
    }
}

/// Las zonas con segundos en el desfase (horas locales de antes de 1900) se
/// truncan al minuto, como moment.
fn offset(now: &Zoned, separator: &str) -> String {
    let seconds = now.offset().seconds();
    let sign = if seconds < 0 { '-' } else { '+' };
    let minutes = seconds.unsigned_abs() / 60;
    format!("{sign}{:02}{separator}{:02}", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use jiff::tz::TimeZone;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn at(instant: &str, zone: &str) -> Zoned {
        let zone = TimeZone::posix(zone).unwrap();
        instant.parse::<Timestamp>().unwrap().to_zoned(zone)
    }

    /// Domingo 18 de octubre de 2026, 15:07:09 en la Ciudad de México.
    fn sunday_afternoon() -> Zoned {
        at("2026-10-18T21:07:09Z", "CST6")
    }

    #[rstest]
    #[case::iso("YYYY-MM-DD", "2026-10-18")]
    #[case::short("YY/M/D", "26/10/18")]
    #[case::names("dddd D [de] MMMM", "domingo 18 de octubre")]
    #[case::abbrs("ddd D MMM", "dom 18 oct")]
    #[case::time24("HH:mm:ss", "15:07:09")]
    #[case::unpadded("H:m:s", "15:7:9")]
    #[case::time12("hh:mm A", "03:07 PM")]
    #[case::time12_lower("h a", "3 pm")]
    #[case::offset("Z", "-06:00")]
    #[case::compact_offset("ZZ", "-0600")]
    #[case::folders("YYYY/MM/YYYY-MM-DD", "2026/10/2026-10-18")]
    #[case::other_letters_are_literal("Do x Q", "18o x Q")]
    #[case::brackets_escape_codes("[YYYY] YYYY", "YYYY 2026")]
    #[case::empty_brackets("YYYY[]MM", "202610")]
    #[case::unicode("D·MMMM ñ", "18·octubre ñ")]
    fn codes(#[case] format: &str, #[case] expected: &str) {
        let format = DateFormat::parse(format).unwrap();
        assert_eq!(format.format(&sunday_afternoon()), expected);
    }

    #[rstest]
    #[case::empty("", FormatError::Empty)]
    #[case::unclosed("YYYY [de", FormatError::UnclosedBracket { at: 5 })]
    fn invalid(#[case] format: &str, #[case] error: FormatError) {
        assert_eq!(DateFormat::parse(format), Err(error));
    }

    #[rstest]
    #[case::midnight("2026-10-18T06:00:00Z", "12 AM 00")]
    #[case::noon("2026-10-18T18:00:00Z", "12 PM 12")]
    fn twelve_hour_clock(#[case] instant: &str, #[case] expected: &str) {
        let format = DateFormat::parse("h A HH").unwrap();
        assert_eq!(format.format(&at(instant, "CST6")), expected);
    }

    #[test]
    fn the_same_instant_is_another_day_in_another_zone() {
        let instant = "2026-10-18T03:30:00Z";
        let format = DateFormat::parse("YYYY-MM-DD HH:mm Z").unwrap();
        let cases = [
            ("UTC0", "2026-10-18 03:30 +00:00"),
            ("CST6", "2026-10-17 21:30 -06:00"),
            ("<+0530>-5:30", "2026-10-18 09:00 +05:30"),
            ("<+14>-14", "2026-10-18 17:30 +14:00"),
        ];
        for (zone, expected) in cases {
            assert_eq!(format.format(&at(instant, zone)), expected, "{zone}");
        }
    }

    #[test]
    fn the_repeated_hour_of_a_dst_change_differs_in_its_offset() {
        let new_york = "EST5EDT,M3.2.0,M11.1.0";
        let format = DateFormat::parse("HH:mm Z").unwrap();
        let first = at("2026-11-01T05:30:00Z", new_york);
        let second = at("2026-11-01T06:30:00Z", new_york);
        assert_eq!(format.format(&first), "01:30 -04:00");
        assert_eq!(format.format(&second), "01:30 -05:00");
    }

    #[test]
    fn the_defaults_are_iso() {
        let now = sunday_afternoon();
        assert_eq!(DateFormat::date().format(&now), "2026-10-18");
        assert_eq!(DateFormat::time().format(&now), "15:07");
        assert_eq!(DateFormat::date(), DateFormat::parse("YYYY-MM-DD").unwrap());
    }
}
