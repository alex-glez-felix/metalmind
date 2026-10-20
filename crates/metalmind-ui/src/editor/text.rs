//! Los tramos de [`markdown`](super::markdown) como `TextRun` de GPUI:
//! color, peso, cursiva, tachado y fondo, todo del tema.

use gpui_kit::{Font, FontStyle, FontWeight, Hsla, Pixels, StrikethroughStyle, TextRun};
use metalmind_syntax::{Decorations, LineKind, Role, Token};

use super::markdown::Run;
use crate::theme::Theme;

/// El tamaño de la letra y el peso de una línea: un heading es más grande.
pub fn line_font(kind: Option<LineKind>, size: Pixels, theme: &Theme) -> (Pixels, FontWeight) {
    match kind {
        Some(LineKind::Heading(level)) => {
            let index = usize::from(level).saturating_sub(1).min(5);
            let (scale, weight) = theme.headings[index];
            (size * scale, weight)
        }
        _ => (size, theme.text.normal),
    }
}

pub fn text_runs(
    runs: &[Run],
    kind: Option<LineKind>,
    font: &Font,
    weight: FontWeight,
    theme: &Theme,
) -> Vec<TextRun> {
    let syntax = &theme.syntax;
    runs.iter()
        .map(|run| {
            let style = run.style;
            let color = ink(run, kind, theme);
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
                font: Font {
                    weight: if has(Decorations::STRONG) && weight.0 < theme.text.strong.0 {
                        theme.text.strong
                    } else {
                        weight
                    },
                    style: if has(Decorations::EMPHASIS) {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    },
                    ..font.clone()
                },
                color,
                background_color: background,
                underline: None,
                strikethrough: has(Decorations::STRIKE).then_some(StrikethroughStyle {
                    thickness: theme.stroke.text_line,
                    color: Some(color),
                }),
            }
        })
        .collect()
}

fn ink(run: &Run, kind: Option<LineKind>, theme: &Theme) -> Hsla {
    let syntax = &theme.syntax;
    let style = run.style;
    if run.failed {
        return theme.ui.error;
    }
    match style.role {
        Role::Text => match kind {
            Some(LineKind::Heading(_)) => syntax.heading,
            _ if style.decorations.contains(Decorations::STRONG) => syntax.bold,
            _ if style.decorations.contains(Decorations::EMPHASIS) => syntax.italic,
            _ => theme.ui.foreground,
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
