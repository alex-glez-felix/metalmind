//! Los tramos de [`markdown`](super::markdown) como `TextRun` de GPUI:
//! color, peso, cursiva, tachado y fondo, todo del tema.

use gpui_kit::{Font, FontStyle, FontWeight, Hsla, Pixels, StrikethroughStyle, TextRun};
use metalmind_config::SyntaxColors;
use metalmind_syntax::{Decorations, LineKind, Role, Style, Token};

use super::markdown::Run;
use crate::theme::{Gpui, Theme};

/// La letra de las líneas del editor (la del texto, la de cada heading y el
/// peso de la negrita) y sus tintas.
#[derive(Clone, Debug, PartialEq)]
pub struct Faces {
    pub font: Font,
    pub size: Pixels,
    pub headings: [(f32, FontWeight); 6],
    pub normal: FontWeight,
    pub strong: FontWeight,
    inks: Inks,
}

/// Los colores del texto de una nota y el grueso de su tachado.
#[derive(Clone, Debug, PartialEq)]
struct Inks {
    syntax: SyntaxColors<Gpui>,
    foreground: Hsla,
    error: Hsla,
    strike: Pixels,
}

impl Faces {
    pub fn new(font: Font, size: Pixels, theme: &Theme) -> Self {
        Self {
            font,
            size,
            headings: theme.headings,
            normal: theme.text.normal,
            strong: theme.text.strong,
            inks: Inks {
                syntax: theme.syntax,
                foreground: theme.ui.foreground,
                error: theme.ui.error,
                strike: theme.stroke.text_line,
            },
        }
    }

    /// El tamaño de la letra y el peso de una línea: un heading es más
    /// grande.
    pub fn line(&self, kind: Option<LineKind>) -> (Pixels, FontWeight) {
        match kind {
            Some(LineKind::Heading(level)) => {
                let index = usize::from(level).saturating_sub(1).min(5);
                let (scale, weight) = self.headings[index];
                (self.size * scale, weight)
            }
            _ => (self.size, self.normal),
        }
    }

    /// La letra de un tramo con `style` en una línea de peso `weight`: la
    /// negrita y la cursiva cambian de letra, que puede medir distinto.
    pub fn run(&self, style: Style, weight: FontWeight) -> Font {
        let has = |decoration| style.decorations.contains(decoration);
        Font {
            weight: if has(Decorations::STRONG) && weight.0 < self.strong.0 {
                self.strong
            } else {
                weight
            },
            style: if has(Decorations::EMPHASIS) {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            ..self.font.clone()
        }
    }
}

pub fn text_runs(
    runs: &[Run],
    kind: Option<LineKind>,
    faces: &Faces,
    weight: FontWeight,
) -> Vec<TextRun> {
    let inks = &faces.inks;
    let syntax = &inks.syntax;
    runs.iter()
        .map(|run| {
            let style = run.style;
            let color = ink(run, kind, inks);
            let has = |decoration| style.decorations.contains(decoration);
            let background = if has(Decorations::CODE) {
                Some(syntax.code_background)
            } else if has(Decorations::MARK) {
                Some(syntax.highlight)
            } else {
                None
            };
            TextRun {
                len: run.len,
                font: faces.run(style, weight),
                color,
                background_color: background,
                underline: None,
                strikethrough: has(Decorations::STRIKE).then_some(StrikethroughStyle {
                    thickness: inks.strike,
                    color: Some(color),
                }),
            }
        })
        .collect()
}

fn ink(run: &Run, kind: Option<LineKind>, inks: &Inks) -> Hsla {
    let syntax = &inks.syntax;
    let style = run.style;
    if run.failed {
        return inks.error;
    }
    match style.role {
        Role::Text => match kind {
            Some(LineKind::Heading(_)) => syntax.heading,
            _ if style.decorations.contains(Decorations::STRONG) => syntax.bold,
            _ if style.decorations.contains(Decorations::EMPHASIS) => syntax.italic,
            _ => inks.foreground,
        },
        Role::Markup => syntax.markup,
        Role::Heading => syntax.heading,
        Role::Link | Role::Url | Role::Wikilink | Role::Embed if run.unresolved => {
            syntax.link_unresolved
        }
        Role::Link | Role::Url | Role::Wikilink => syntax.link,
        Role::Embed => syntax.embed,
        Role::Tag => syntax.tag,
        Role::Code => syntax.code,
        Role::Quote => syntax.quote,
        Role::ListMarker => syntax.list_marker,
        Role::Checkbox => syntax.checkbox,
        Role::TaskDone => syntax.task_done,
        Role::Comment => syntax.comment,
        Role::BlockId => syntax.block_id,
        Role::Frontmatter => syntax.frontmatter,
        Role::FrontmatterKey => syntax.frontmatter_key,
        Role::FenceInfo => syntax.fence_info,
        Role::Token(Token::Keyword) => syntax.code_keyword,
        Role::Token(Token::String) => syntax.code_string,
        Role::Token(Token::Number) => syntax.code_number,
        Role::Token(Token::Comment) => syntax.code_comment,
        Role::Math => syntax.math,
    }
}
