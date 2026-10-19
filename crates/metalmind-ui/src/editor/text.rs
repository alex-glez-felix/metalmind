//! Los tramos de [`markdown`](super::markdown) como `TextRun` de GPUI:
//! color, peso, cursiva, tachado y fondo, todo del tema.

use gpui_kit::{Font, FontStyle, FontWeight, Hsla, Pixels, StrikethroughStyle, TextRun};
use metalmind_syntax::{Decorations, LineKind, Role, Token};

use super::markdown::Run;
use crate::theme::{Markdown, Theme};

/// El tamaño de la letra y el peso de una línea: un heading es más grande.
pub fn line_font(kind: Option<LineKind>, size: Pixels, theme: &Theme) -> (Pixels, FontWeight) {
    match kind {
        Some(LineKind::Heading(level)) => {
            let index = usize::from(level).saturating_sub(1).min(5);
            let (scale, weight) = theme.markdown.headings[index];
            (size * scale, weight)
        }
        _ => (size, theme.weight_normal),
    }
}

pub fn text_runs(
    runs: &[Run],
    kind: Option<LineKind>,
    font: &Font,
    weight: FontWeight,
    theme: &Theme,
) -> Vec<TextRun> {
    let markdown = &theme.markdown;
    runs.iter()
        .map(|run| {
            let style = run.style;
            let color = ink(run, kind, theme);
            let has = |decoration| style.decorations.contains(decoration);
            let background = if has(Decorations::CODE) {
                Some(markdown.code_background)
            } else if has(Decorations::MARK) {
                Some(markdown.highlight)
            } else {
                None
            };
            TextRun {
                len: run.len,
                font: Font {
                    weight: if has(Decorations::STRONG) && weight.0 < theme.weight_strong.0 {
                        theme.weight_strong
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
                    thickness: markdown.strike_thickness,
                    color: Some(color),
                }),
            }
        })
        .collect()
}

fn ink(run: &Run, kind: Option<LineKind>, theme: &Theme) -> Hsla {
    let markdown: &Markdown = &theme.markdown;
    let style = run.style;
    if run.failed {
        return theme.error;
    }
    match style.role {
        Role::Text => match kind {
            Some(LineKind::Heading(_)) => markdown.heading,
            _ if style.decorations.contains(Decorations::STRONG) => markdown.bold,
            _ if style.decorations.contains(Decorations::EMPHASIS) => markdown.italic,
            _ => theme.foreground,
        },
        Role::Markup => markdown.markup,
        Role::Heading => markdown.heading,
        Role::Link | Role::Url | Role::Wikilink | Role::Embed if run.unresolved => {
            markdown.link_unresolved
        }
        Role::Link | Role::Url | Role::Wikilink => markdown.link,
        Role::Embed => markdown.embed,
        Role::Tag => markdown.tag,
        Role::Code => markdown.code,
        Role::Quote => markdown.quote,
        Role::ListMarker => markdown.list_marker,
        Role::Checkbox => markdown.checkbox,
        Role::TaskDone => markdown.task_done,
        Role::Comment => markdown.comment,
        Role::BlockId => markdown.block_id,
        Role::Frontmatter => markdown.frontmatter,
        Role::FrontmatterKey => markdown.frontmatter_key,
        Role::FenceInfo => markdown.fence_info,
        Role::Token(Token::Keyword) => markdown.code_keyword,
        Role::Token(Token::String) => markdown.code_string,
        Role::Token(Token::Number) => markdown.code_number,
        Role::Token(Token::Comment) => markdown.code_comment,
        Role::Math => markdown.math,
    }
}
