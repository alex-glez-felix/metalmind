//! Las variables de una plantilla: `{{date}}`, `{{time}}`, `{{title}}` y
//! `{{date:FORMATO}}` o `{{time:FORMATO}}`, las mismas de Obsidian. No hay
//! más lenguaje que eso: lo que no es una variable se copia tal cual.

use std::ops::Range;

use jiff::Zoned;

use crate::format::{DateFormat, FormatError};

/// Lo que llena las variables.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub title: &'a str,
    pub now: &'a Zoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    /// Variables que se dejaron tal cual, en orden.
    pub problems: Vec<VariableProblem>,
}

/// Una variable mal escrita. Los rangos son bytes de la plantilla.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VariableProblem {
    #[error("`{text}` no es una variable de plantilla")]
    Unknown { span: Range<usize>, text: String },
    #[error("el `{{{{` del byte {at} no se cierra en su línea")]
    Unclosed { at: usize },
    #[error("`{text}`: {error}")]
    InvalidFormat {
        span: Range<usize>,
        text: String,
        error: FormatError,
    },
}

/// Llena las variables de `template`. Lo que se inserta no se vuelve a
/// leer: un título con `{{date}}` queda escrito así.
#[must_use]
pub fn render(template: &str, context: Context) -> Rendered {
    let mut text = String::with_capacity(template.len());
    let mut problems = Vec::new();
    let mut copied = 0;
    let mut from = 0;
    while let Some(found) = template[from..].find("{{") {
        let open = last_open(template, from + found);
        let inner_start = open + 2;
        let Some(close) = close_of(template, inner_start) else {
            problems.push(VariableProblem::Unclosed { at: open });
            from = inner_start;
            continue;
        };
        let span = open..close + 2;
        match expand(&template[inner_start..close], context) {
            Ok(value) => {
                text.push_str(&template[copied..open]);
                text.push_str(&value);
                copied = span.end;
            }
            Err(problem) => problems.push(problem.at(span.clone(), &template[span.clone()])),
        }
        from = span.end;
    }
    text.push_str(&template[copied..]);
    Rendered { text, problems }
}

/// En `{{{date}}}` la variable es la del último par de llaves; las de antes
/// son texto.
fn last_open(template: &str, open: usize) -> usize {
    let extra = template[open + 2..]
        .bytes()
        .take_while(|&byte| byte == b'{')
        .count();
    open + extra
}

/// Dónde está el `}}` que cierra, si llega antes del fin de línea y de otro `{{`.
fn close_of(template: &str, inner_start: usize) -> Option<usize> {
    let rest = &template[inner_start..];
    let line = &rest[..rest.find('\n').unwrap_or(rest.len())];
    let close = line.find("}}")?;
    line[..close]
        .find("{{")
        .is_none()
        .then_some(inner_start + close)
}

enum Failure {
    Unknown,
    InvalidFormat(FormatError),
}

impl Failure {
    fn at(self, span: Range<usize>, text: &str) -> VariableProblem {
        let text = text.to_owned();
        match self {
            Self::Unknown => VariableProblem::Unknown { span, text },
            Self::InvalidFormat(error) => VariableProblem::InvalidFormat { span, text, error },
        }
    }
}

fn expand(inner: &str, context: Context) -> Result<String, Failure> {
    let inner = inner.trim();
    let (name, format) = match inner.split_once(':') {
        Some((name, format)) => (name, Some(format)),
        None => (inner, None),
    };
    let format = match (name, format) {
        ("title", None) => return Ok(context.title.to_owned()),
        ("date", None) => DateFormat::date(),
        ("time", None) => DateFormat::time(),
        ("date" | "time", Some(format)) => {
            DateFormat::parse(format).map_err(Failure::InvalidFormat)?
        }
        _ => return Err(Failure::Unknown),
    };
    Ok(format.format(context.now))
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use jiff::tz::TimeZone;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn now() -> Zoned {
        let zone = TimeZone::posix("CST6").unwrap();
        "2026-10-18T21:07:09Z"
            .parse::<Timestamp>()
            .unwrap()
            .to_zoned(zone)
    }

    fn rendered(template: &str, title: &str) -> Rendered {
        render(template, Context { title, now: &now() })
    }

    #[rstest]
    #[case::date("{{date}}", "2026-10-18")]
    #[case::time("{{time}}", "15:07")]
    #[case::title("# {{title}}", "# Reunión")]
    #[case::date_format("{{date:dddd D [de] MMMM}}", "domingo 18 de octubre")]
    #[case::time_format("{{time:HH:mm:ss}}", "15:07:09")]
    #[case::colons_in_the_format("{{date:HH:mm}}", "15:07")]
    #[case::spaces_inside("{{ title }} {{ date }}", "Reunión 2026-10-18")]
    #[case::several("{{date}}{{time}}", "2026-10-1815:07")]
    #[case::extra_braces("{{{title}}}", "{Reunión}")]
    #[case::frontmatter(
        "---\ncreado: {{date}}\n---\n# {{title}}\n",
        "---\ncreado: 2026-10-18\n---\n# Reunión\n"
    )]
    #[case::unicode("ñ {{title}} ü", "ñ Reunión ü")]
    #[case::no_variables("texto { } }} sin nada", "texto { } }} sin nada")]
    fn variables(#[case] template: &str, #[case] expected: &str) {
        let rendered = rendered(template, "Reunión");
        assert_eq!(rendered.problems, []);
        assert_eq!(rendered.text, expected);
    }

    #[test]
    fn the_title_is_not_read_again() {
        let rendered = rendered("{{title}}", "{{date}}");
        assert_eq!(rendered.text, "{{date}}");
        assert_eq!(rendered.problems, []);
    }

    #[test]
    fn an_empty_title() {
        assert_eq!(rendered("# {{title}}", "").text, "# ");
    }

    fn unknown(span: Range<usize>, text: &str) -> VariableProblem {
        VariableProblem::Unknown {
            span,
            text: text.to_owned(),
        }
    }

    #[rstest]
    #[case::unknown_name("a {{autor}} b", vec![unknown(2..11, "{{autor}}")])]
    #[case::case_matters("{{Date}}", vec![unknown(0..8, "{{Date}}")])]
    #[case::empty("{{}}", vec![unknown(0..4, "{{}}")])]
    #[case::title_with_format("{{title:x}}", vec![unknown(0..11, "{{title:x}}")])]
    #[case::unclosed("{{date", vec![VariableProblem::Unclosed { at: 0 }])]
    #[case::single_brace("{{date}", vec![VariableProblem::Unclosed { at: 0 }])]
    #[case::other_line("{{date\n}}", vec![VariableProblem::Unclosed { at: 0 }])]
    #[case::invalid_format(
        "{{date:}} {{date:[x}}",
        vec![
            VariableProblem::InvalidFormat {
                span: 0..9,
                text: "{{date:}}".to_owned(),
                error: FormatError::Empty,
            },
            VariableProblem::InvalidFormat {
                span: 10..21,
                text: "{{date:[x}}".to_owned(),
                error: FormatError::UnclosedBracket { at: 0 },
            },
        ]
    )]
    fn misspelled_variables_stay_as_written(
        #[case] template: &str,
        #[case] problems: Vec<VariableProblem>,
    ) {
        let rendered = rendered(template, "t");
        assert_eq!(rendered.text, template);
        assert_eq!(rendered.problems, problems);
    }

    #[test]
    fn an_unclosed_variable_does_not_hide_the_next_one() {
        let rendered = rendered("{{date {{title}}", "t");
        assert_eq!(rendered.text, "{{date t");
        assert_eq!(rendered.problems, [VariableProblem::Unclosed { at: 0 }]);
    }

    #[test]
    fn spans_are_bytes() {
        let rendered = rendered("ñandú {{x}}", "t");
        assert_eq!(rendered.problems, [unknown(8..13, "{{x}}")]);
    }
}
