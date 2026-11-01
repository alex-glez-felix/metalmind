//! Cómo se ve cada fila de las listas: un tag con su ícono, el alias por el
//! que coincidió y su conteo; una nota con su nombre y su carpeta; una
//! carpeta con su ruta; un archivo de `:e` con su nombre y su carpeta; un
//! comando de `:` con su acción.

use std::ops::Range;

use gpui_kit::{Div, ParentElement as _, Styled as _};
use metalmind_syntax::note_name;
use metalmind_vault::{EntryKind, VaultPath};

use super::super::completion_menu::MenuRow;
use crate::completion::{
    ArgumentCompletions, ArgumentList, CommandCompletions, LinkCompletions, TagCompletions,
    ViewCompletions,
};
use crate::icons::{self, Icon};
use crate::theme::Theme;

pub(super) fn tags(list: &TagCompletions, visible: Range<usize>, theme: &Theme) -> Vec<MenuRow> {
    list.rows()
        .skip(visible.start)
        .take(visible.len())
        .map(|(completion, alias)| MenuRow {
            icon: icons::tag(completion.icon.as_ref(), theme),
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
                icon: icons::entry(EntryKind::of_file(path), false, theme),
                name: note_name::strip_extension(name).unwrap_or(name).to_owned(),
                detail: folder_of(path),
                count: None,
            }
        })
        .collect()
}

/// Una línea del historial o un nombre, tras un `:` tenue; el de un
/// comando ajeno, con la acción que corre.
pub(super) fn commands(
    list: &CommandCompletions,
    visible: Range<usize>,
    theme: &Theme,
) -> Vec<MenuRow> {
    visible
        .filter_map(|row| list.row(row))
        .map(|row| MenuRow {
            icon: muted_glyph(":", theme),
            name: row.text.to_owned(),
            detail: row.action.map(str::to_owned),
            count: None,
        })
        .collect()
}

/// Una ruta se ve como se lee, sin los escapes con que se escribe: una
/// carpeta entera; un archivo, su nombre y, apagada, su carpeta, así lo
/// último que se corta es el nombre.
pub(super) fn argument(
    list: &ArgumentCompletions,
    visible: Range<usize>,
    theme: &Theme,
) -> Vec<MenuRow> {
    let (paths, folders) = match list.list() {
        ArgumentList::Tags { list, .. } => return tags(list, visible, theme),
        ArgumentList::Views(list) => return views(list, visible, theme),
        ArgumentList::Folders(folders) => (folders, true),
        ArgumentList::Files(files) => (files, false),
    };
    let rows = paths.rows().skip(visible.start).take(visible.len());
    if folders {
        rows.map(|folder| MenuRow {
            icon: icons::entry(EntryKind::Folder, false, theme),
            name: format!("{folder}/"),
            detail: None,
            count: None,
        })
        .collect()
    } else {
        rows.map(|file| MenuRow {
            icon: icons::entry(EntryKind::of_file(file), false, theme),
            name: file.name().to_owned(),
            detail: folder_of(file),
            count: None,
        })
        .collect()
    }
}

/// La carpeta de `path`, salvo la raíz.
fn folder_of(path: &VaultPath) -> Option<String> {
    path.parent()
        .filter(|parent| !parent.is_root())
        .map(|parent| parent.as_str().to_owned())
}

fn views(list: &ViewCompletions, visible: Range<usize>, theme: &Theme) -> Vec<MenuRow> {
    list.rows()
        .skip(visible.start)
        .take(visible.len())
        .map(|view| MenuRow {
            icon: icons::icon(Icon::Bookmark, theme.ui.panel_muted, theme),
            name: view.name.clone(),
            detail: Some(view.query.clone()),
            count: None,
        })
        .collect()
}

fn muted_glyph(glyph: &'static str, theme: &Theme) -> Div {
    icons::slot(theme)
        .text_color(theme.ui.panel_muted)
        .child(glyph)
}
