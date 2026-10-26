//! La nota del día: dónde va y con qué nace. Apagada salvo que
//! `daily.enabled` diga lo contrario (06 §11).

use jiff::Zoned;
use metalmind_config::Daily;
use metalmind_syntax::note_name;
use metalmind_vault::tags::TagGraph;
use metalmind_vault::{InvalidPath, VaultPath};

use crate::draft::{Draft, NewNote, Pick, Templates, draft};
use crate::format::{DateFormat, FormatError};
use crate::note_path::NotePath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyNote {
    pub path: VaultPath,
    /// El texto con el que se crea si todavía no existe.
    pub draft: Draft,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DailyError {
    #[error("`daily.format`: {0}")]
    Format(#[from] FormatError),
    #[error("la nota del día no tiene una ruta válida: {0}")]
    Path(#[from] InvalidPath),
}

/// La nota del día de `now`, en su zona horaria: pasada la medianoche local
/// ya es la de mañana. `None` si las notas del día están apagadas.
///
/// # Errors
/// Si `daily.format` no es un formato válido o la ruta que da no es una
/// ruta del vault.
pub fn daily_note(
    daily: &Daily,
    now: &Zoned,
    templates: &Templates,
) -> Result<Option<DailyNote>, DailyError> {
    if !daily.enabled {
        return Ok(None);
    }
    let name = DateFormat::parse(&daily.format)?.format(now);
    // Solo se quita un `/` final: `/` o `//` tienen que fallar como absolutas.
    let folder = daily.folder.strip_suffix('/').unwrap_or(&daily.folder);
    let path = if daily.folder.is_empty() {
        note_name::with_extension(&name)
    } else {
        note_name::with_extension(&format!("{folder}/{name}"))
    };
    let NotePath { path, title } = NotePath::from_file(VaultPath::new(&path)?);
    let pick = daily.template.as_deref().map_or(Pick::Blank, Pick::Named);
    let note = NewNote {
        title: &title,
        tags: &[],
        pick,
        now,
    };
    Ok(Some(DailyNote {
        path,
        draft: draft(&note, templates, &TagGraph::default()),
    }))
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use jiff::tz::TimeZone;
    use metalmind_vault::InvalidReason;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;
    use crate::draft::DraftProblem;

    fn at(instant: &str, zone: &str) -> Zoned {
        let zone = TimeZone::posix(zone).unwrap();
        instant.parse::<Timestamp>().unwrap().to_zoned(zone)
    }

    fn enabled() -> Daily {
        Daily {
            enabled: true,
            ..Daily::default()
        }
    }

    fn templates() -> Templates {
        Templates::new([(
            "diario".to_owned(),
            "# {{title}}\n{{date:dddd}}, {{time}}\n".to_owned(),
        )])
    }

    fn path(daily: &Daily, now: &Zoned) -> String {
        daily_note(daily, now, &templates())
            .unwrap()
            .unwrap()
            .path
            .as_str()
            .to_owned()
    }

    #[test]
    fn off_by_default() {
        let now = at("2026-10-18T12:00:00Z", "UTC0");
        assert_eq!(daily_note(&Daily::default(), &now, &templates()), Ok(None));
    }

    #[rstest]
    #[case::default("diario", "YYYY-MM-DD", "diario/2026-10-18.md")]
    #[case::root("", "YYYY-MM-DD", "2026-10-18.md")]
    #[case::trailing_slash("diario/", "YYYY-MM-DD", "diario/2026-10-18.md")]
    #[case::nested_folder("notas/diario", "YYYY-MM-DD", "notas/diario/2026-10-18.md")]
    #[case::subfolders_from_format("diario", "YYYY/MM/DD", "diario/2026/10/18.md")]
    #[case::names("", "dddd D [de] MMMM", "domingo 18 de octubre.md")]
    fn paths(#[case] folder: &str, #[case] format: &str, #[case] expected: &str) {
        let daily = Daily {
            folder: folder.to_owned(),
            format: format.to_owned(),
            ..enabled()
        };
        let now = at("2026-10-18T12:00:00Z", "UTC0");
        assert_eq!(path(&daily, &now), expected);
    }

    #[rstest]
    #[case::utc("UTC0", "diario/2026-10-18.md")]
    #[case::mexico_city_is_still_yesterday("CST6", "diario/2026-10-17.md")]
    #[case::kiritimati_is_already_tomorrow("<+14>-14", "diario/2026-10-18.md")]
    #[case::kolkata("<+0530>-5:30", "diario/2026-10-18.md")]
    fn the_day_is_the_local_one(#[case] zone: &str, #[case] expected: &str) {
        let now = at("2026-10-18T03:30:00Z", zone);
        assert_eq!(path(&enabled(), &now), expected);
    }

    #[test]
    fn just_before_and_after_local_midnight() {
        let before = at("2026-10-19T05:59:59Z", "CST6");
        let after = at("2026-10-19T06:00:00Z", "CST6");
        assert_eq!(path(&enabled(), &before), "diario/2026-10-18.md");
        assert_eq!(path(&enabled(), &after), "diario/2026-10-19.md");
    }

    #[test]
    fn the_template_fills_the_title_with_the_note_name() {
        let daily = Daily {
            format: "YYYY/MM/YYYY-MM-DD".to_owned(),
            template: Some("diario".to_owned()),
            ..enabled()
        };
        let now = at("2026-10-18T21:07:00Z", "CST6");
        let note = daily_note(&daily, &now, &templates()).unwrap().unwrap();
        assert_eq!(note.path.as_str(), "diario/2026/10/2026-10-18.md");
        assert_eq!(note.draft.text, "# 2026-10-18\ndomingo, 15:07\n");
        assert_eq!(note.draft.problems, []);
    }

    #[test]
    fn without_template_it_starts_empty() {
        let now = at("2026-10-18T12:00:00Z", "UTC0");
        let note = daily_note(&enabled(), &now, &templates()).unwrap().unwrap();
        assert_eq!(note.draft.text, "");
        assert_eq!(note.draft.template, None);
    }

    #[test]
    fn a_missing_template_is_reported_and_the_note_starts_empty() {
        let daily = Daily {
            template: Some("no-existe".to_owned()),
            ..enabled()
        };
        let now = at("2026-10-18T12:00:00Z", "UTC0");
        let note = daily_note(&daily, &now, &templates()).unwrap().unwrap();
        assert_eq!(note.draft.text, "");
        assert_eq!(
            note.draft.problems,
            [DraftProblem::MissingTemplate {
                name: "no-existe".to_owned(),
                tag: None
            }]
        );
    }

    #[rstest]
    #[case::empty_format("diario", "", DailyError::Format(FormatError::Empty))]
    #[case::unclosed(
        "diario",
        "YYYY [x",
        DailyError::Format(FormatError::UnclosedBracket { at: 5 })
    )]
    #[case::absolute("/diario", "YYYY", invalid("/diario/2026.md", InvalidReason::Absolute))]
    #[case::slash("/", "YYYY", invalid("/2026.md", InvalidReason::Absolute))]
    #[case::two_slashes("//", "YYYY", invalid("//2026.md", InvalidReason::Absolute))]
    #[case::escapes("..", "YYYY", invalid("../2026.md", InvalidReason::Relative))]
    #[case::hidden("", "[.]YYYY", invalid(".2026.md", InvalidReason::Hidden))]
    #[case::empty_component("", "YYYY//MM", invalid("2026//10.md", InvalidReason::EmptyComponent))]
    fn invalid_settings(#[case] folder: &str, #[case] format: &str, #[case] error: DailyError) {
        let daily = Daily {
            folder: folder.to_owned(),
            format: format.to_owned(),
            ..enabled()
        };
        let now = at("2026-10-18T12:00:00Z", "UTC0");
        assert_eq!(daily_note(&daily, &now, &templates()), Err(error));
    }

    fn invalid(path: &str, reason: InvalidReason) -> DailyError {
        DailyError::Path(InvalidPath {
            path: path.to_owned(),
            reason,
        })
    }
}
