//! Cómo se ve la vista de backlinks del panel: cada nota que enlaza a la
//! activa, con su carpeta y cuántos links tiene; debajo, la línea de cada
//! link con el link resaltado.

use std::ops::Range;

use gpui_kit::{
    AnyElement, Context, Div, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, StyledText,
    div, uniform_list,
};
use metalmind_syntax::note_name;
use metalmind_vault::{EntryKind, LinkContext, VaultPath};
use unicode_width::UnicodeWidthChar as _;

use super::render::row_frame;
use super::{Sidebar, icons};
use crate::backlinks::Row;
use crate::panel::{Outline as _, RowMarks};
use crate::theme::Theme;

/// Cuántas columnas de la línea se ven antes del link, como mucho: en un
/// panel angosto, el link tiene que entrar.
const LEAD_COLUMNS: usize = 10;

impl Sidebar {
    pub(super) fn backlink_body(
        &self,
        theme: &Theme,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let backlinks = &self.panel.backlinks;
        if backlinks.rows().is_empty() {
            let text = match backlinks.note() {
                Some(note) => format!("Ninguna nota enlaza a {}.", note.name()),
                None => "La nota activa no es del vault.".to_owned(),
            };
            return div()
                .flex_1()
                .px(theme.layout.panel_padding_x)
                .py(theme.layout.panel_padding_y)
                .text_color(theme.ui.panel_muted)
                .child(text)
                .into_any_element();
        }
        let theme = theme.clone();
        uniform_list(
            "backlinks",
            backlinks.rows().len(),
            cx.processor(move |sidebar, range: Range<usize>, _, cx| {
                sidebar.backlink_rows(range, focused, &theme, cx)
            }),
        )
        .track_scroll(&self.backlink_scroll)
        .flex_1()
        .into_any_element()
    }

    fn backlink_rows(
        &self,
        range: Range<usize>,
        focused: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let selected = self.panel.backlinks.selected_row();
        self.panel.backlinks.rows()[range.clone()]
            .iter()
            .zip(range)
            .map(|(row, at)| {
                let marks = RowMarks::of(selected == Some(at), focused, false);
                backlink_row(row, at, marks, theme, cx)
            })
            .collect()
    }
}

fn backlink_row(
    row: &Row,
    at: usize,
    marks: RowMarks,
    theme: &Theme,
    cx: &mut Context<Sidebar>,
) -> AnyElement {
    match row {
        Row::Note {
            source,
            count,
            expanded,
            ..
        } => row_frame(at, 0, marks, theme, cx)
            .text_color(theme.ui.panel_foreground)
            .child(disclosure(at, *expanded, theme, cx))
            .child(icons::entry(EntryKind::Note, theme))
            .child(note_label(source, theme))
            .child(muted(theme).flex_none().child(count.to_string())),
        Row::Link { context, .. } => row_frame(at, 1, marks, theme, cx)
            .text_color(theme.ui.panel_foreground)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(theme.ui_small_font_size)
                    .child(context_text(context, theme)),
            ),
    }
    .into_any_element()
}

/// `▾` o `▸`: un clic abre o cierra la nota sin abrirla en el editor.
fn disclosure(at: usize, expanded: bool, theme: &Theme, cx: &mut Context<Sidebar>) -> AnyElement {
    icons::arrow(Some(expanded), theme)
        .id(("backlink-disclosure", at))
        .on_click(cx.listener(move |sidebar, _, window, cx| {
            cx.stop_propagation();
            sidebar.toggle_backlink_note(at, window, cx);
        }))
        .into_any_element()
}

/// El nombre sin `.md` y, tenue, su carpeta, que se recorta primero.
fn note_label(source: &VaultPath, theme: &Theme) -> Div {
    let name = source.name();
    let folder = source.parent().filter(|parent| !parent.is_root());
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .gap(theme.layout.panel_gap)
        .child(
            div()
                .min_w_0()
                .truncate()
                .child(note_name::strip_extension(name).unwrap_or(name).to_owned()),
        )
        .children(folder.map(|folder| {
            muted(theme)
                .flex_1()
                .min_w_0()
                .truncate()
                .child(folder.as_str().to_owned())
        }))
}

fn muted(theme: &Theme) -> Div {
    div()
        .text_size(theme.ui_small_font_size)
        .text_color(theme.ui.panel_muted)
}

/// La línea del link desde poco antes de él, con el link en el color de
/// los links del editor.
fn context_text(context: &LinkContext, theme: &Theme) -> StyledText {
    let (text, link) = lead_in(context);
    let style = HighlightStyle {
        color: Some(theme.syntax.link),
        ..HighlightStyle::default()
    };
    StyledText::new(SharedString::from(text)).with_highlights([(link, style)])
}

/// El texto desde [`LEAD_COLUMNS`] columnas antes del link (desde la
/// palabra siguiente, si el corte cae en una), con `…` si se cortó algo, y
/// dónde queda el link en él. Un carácter ancho, como los del japonés,
/// cuenta dos.
fn lead_in(context: &LinkContext) -> (String, Range<usize>) {
    let before = &context.text[..context.link.start];
    let mut start = before.len();
    let mut columns = 0;
    for (at, c) in before.char_indices().rev() {
        columns += c.width().unwrap_or(0);
        if columns > LEAD_COLUMNS {
            break;
        }
        start = at;
    }
    if start == 0 {
        return (context.text.clone(), context.link.clone());
    }
    let mid_word = !before[..start].ends_with(char::is_whitespace);
    if mid_word && let Some(space) = before[start..].find(char::is_whitespace) {
        start += space
            + before[start + space..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
    }
    let text = format!("…{}", &context.text[start..]);
    let link_start = '…'.len_utf8() + context.link.start - start;
    (text, link_start..link_start + context.link.len())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn context(text: &str, link: &str) -> LinkContext {
        let start = text.find(link).unwrap();
        LinkContext {
            text: text.to_owned(),
            link: start..start + link.len(),
        }
    }

    #[rstest::rstest]
    #[case::corta("ver el [[plan]] hoy", "ver el [[plan]] hoy")]
    #[case::larga(
        "una línea larguísima que habla de muchas cosas antes de nombrar el [[plan]] al fin",
        "…el [[plan]] al fin"
    )]
    #[case::justo("0123456789[[plan]]", "0123456789[[plan]]")]
    #[case::multibyte("ñññññññññññññññññ[[plan]]", "…ññññññññññ[[plan]]")]
    #[case::una_palabra_larga_pegada("x 0123456789abcdefghij[[plan]]", "…abcdefghij[[plan]]")]
    #[case::caracteres_anchos("日本語のメモ:計画を見直す [[plan]] とても", "…[[plan]] とても")]
    #[case::anchos_sin_espacio("日本語のメモ計画を見直す[[plan]]", "…画を見直す[[plan]]")]
    fn el_link_queda_cerca_del_principio(#[case] line: &str, #[case] shown: &str) {
        let (text, link) = lead_in(&context(line, "[[plan]]"));
        assert_eq!(text, shown);
        assert_eq!(&text[link], "[[plan]]");
    }
}
