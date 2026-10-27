//! Cómo se ve cada fila de las listas: un tag con su ícono, el alias por el
//! que coincidió y su conteo; una nota con su nombre y su carpeta; una
//! carpeta con su ruta.

use std::ops::Range;

use gpui_kit::{Div, ParentElement as _, Styled as _, div};
use metalmind_syntax::note_name;
use metalmind_vault::EntryKind;
use metalmind_vault::tags::TagIcon;

use super::super::completion_menu::MenuRow;
use crate::completion::{ArgumentCompletions, ArgumentList, LinkCompletions, TagCompletions};
use crate::sidebar::icons;
use crate::theme::Theme;

pub(super) fn tags(list: &TagCompletions, visible: Range<usize>, theme: &Theme) -> Vec<MenuRow> {
    list.rows()
        .skip(visible.start)
        .take(visible.len())
        .map(|(completion, alias)| MenuRow {
            icon: tag_icon(completion.icon.as_ref(), theme),
            name: completion.name.clone(),
            detail: alias.map(|alias| format!("alias {alias}")),
            count: Some(completion.count.to_string()),
        })
        .collect()
}

pub(super) fn links(list: &LinkCompletions, visible: Range<usize>, theme: &Theme) -> Vec<MenuRow> {
    list.rows()
        .skip(visible.start)
        .take(visible.len())
        .map(|path| {
            let name = path.name();
            MenuRow {
                icon: icons::entry(EntryKind::of_file(path), theme),
                name: note_name::strip_extension(name).unwrap_or(name).to_owned(),
                detail: path
                    .parent()
                    .filter(|parent| !parent.is_root())
                    .map(|parent| parent.as_str().to_owned()),
                count: None,
            }
        })
        .collect()
}

/// Una carpeta se ve como se lee, sin los escapes con que se escribe.
pub(super) fn argument(
    list: &ArgumentCompletions,
    visible: Range<usize>,
    theme: &Theme,
) -> Vec<MenuRow> {
    let folders = match list.list() {
        ArgumentList::Tags { list, .. } => return tags(list, visible, theme),
        ArgumentList::Folders(folders) => folders,
    };
    folders
        .rows()
        .skip(visible.start)
        .take(visible.len())
        .map(|folder| MenuRow {
            icon: icons::entry(EntryKind::Folder, theme),
            name: format!("{folder}/"),
            detail: None,
            count: None,
        })
        .collect()
}

/// El emoji de la página del tag o un `#` tenue, como en el panel.
fn tag_icon(icon: Option<&TagIcon>, theme: &Theme) -> Div {
    let slot = div().w(theme.icon_size).flex_none().flex().justify_center();
    match icon {
        Some(TagIcon::Emoji(emoji)) => slot.child(emoji.clone()),
        Some(TagIcon::Set { .. }) | None => slot.text_color(theme.ui.panel_muted).child("#"),
    }
}
