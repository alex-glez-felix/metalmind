//! Los íconos de la UI, todos de Tabler (05 §7): de línea, con el mismo
//! trazo, en un hueco cuadrado de `theme.icon_size` y en un color del tema.
//! GPUI los pide a [`Assets`] por su ruta y los pinta como máscara.

mod tabler;

use std::borrow::Cow;

use gpui_kit::{AssetSource, Div, Hsla, ParentElement as _, SharedString, Styled as _, div, svg};
use metalmind_vault::EntryKind;
use metalmind_vault::tags::TagIcon;

use crate::theme::Theme;
use tabler::Tabler;

const TABLER_DIR: &str = "icons/tabler/";
const SVG_EXTENSION: &str = ".svg";

/// Los íconos propios de la UI, cada uno con su nombre en Tabler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Folder,
    FolderOpen,
    Note,
    Attachment,
    Collapsed,
    Expanded,
    Tags,
    Hash,
    Inbox,
    Backlinks,
    NoteList,
}

impl Icon {
    #[cfg(test)]
    const ALL: [Self; 11] = [
        Self::Folder,
        Self::FolderOpen,
        Self::Note,
        Self::Attachment,
        Self::Collapsed,
        Self::Expanded,
        Self::Tags,
        Self::Hash,
        Self::Inbox,
        Self::Backlinks,
        Self::NoteList,
    ];

    /// La ruta que GPUI le pide a [`Assets`].
    const fn path(self) -> &'static str {
        match self {
            Self::Folder => "icons/tabler/folder.svg",
            Self::FolderOpen => "icons/tabler/folder-open.svg",
            Self::Note => "icons/tabler/file-text.svg",
            Self::Attachment => "icons/tabler/file.svg",
            Self::Collapsed => "icons/tabler/chevron-right.svg",
            Self::Expanded => "icons/tabler/chevron-down.svg",
            Self::Tags => "icons/tabler/tags.svg",
            Self::Hash => "icons/tabler/hash.svg",
            Self::Inbox => "icons/tabler/inbox.svg",
            Self::Backlinks => "icons/tabler/arrow-back-up.svg",
            Self::NoteList => "icons/tabler/list.svg",
        }
    }
}

/// El ícono en su hueco, en `color`.
pub fn icon(icon: Icon, color: Hsla, theme: &Theme) -> Div {
    from_path(icon.path(), color, theme)
}

fn from_path(path: impl Into<SharedString>, color: Hsla, theme: &Theme) -> Div {
    slot(theme).child(
        svg()
            .path(path)
            .size(theme.icon_size)
            .flex_none()
            .text_color(color),
    )
}

/// El hueco de un ícono, también para un emoji o para nada.
pub fn slot(theme: &Theme) -> Div {
    div()
        .size(theme.icon_size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
}

/// Una carpeta, una nota o un adjunto, cada uno en su color del panel.
pub fn entry(kind: EntryKind, expanded: bool, theme: &Theme) -> Div {
    match kind {
        EntryKind::Folder if expanded => icon(Icon::FolderOpen, theme.ui.panel_folder, theme),
        EntryKind::Folder => icon(Icon::Folder, theme.ui.panel_folder, theme),
        EntryKind::Note => icon(Icon::Note, theme.ui.panel_note, theme),
        EntryKind::Attachment => icon(Icon::Attachment, theme.ui.panel_muted, theme),
    }
}

/// El chevron delante de una carpeta; un hueco del mismo ancho delante de
/// un archivo, para que los nombres queden alineados.
pub fn disclosure(kind: EntryKind, expanded: bool, theme: &Theme) -> Div {
    arrow((kind == EntryKind::Folder).then_some(expanded), theme)
}

/// El chevron cerrado o abierto, o el hueco de lo que no se abre.
pub fn arrow(expanded: Option<bool>, theme: &Theme) -> Div {
    match expanded {
        Some(true) => icon(Icon::Expanded, theme.ui.panel_muted, theme),
        Some(false) => icon(Icon::Collapsed, theme.ui.panel_muted, theme),
        None => slot(theme),
    }
}

/// El ícono de la página del tag: su emoji, o su ícono de Tabler en el
/// color del texto. Sin ícono, o con uno que no está, un `#` tenue (05 §7).
pub fn tag(icon: Option<&TagIcon>, theme: &Theme) -> Div {
    match icon {
        Some(TagIcon::Emoji(emoji)) => slot(theme).child(emoji.clone()),
        Some(TagIcon::Set { set, name }) if set == "tabler" && in_tabler(name) => from_path(
            format!("{TABLER_DIR}{name}{SVG_EXTENSION}"),
            theme.ui.panel_foreground,
            theme,
        ),
        Some(TagIcon::Set { .. }) | None => self::icon(Icon::Hash, theme.ui.panel_muted, theme),
    }
}

fn in_tabler(name: &str) -> bool {
    Tabler::get().is_ok_and(|set| set.contains(name))
}

/// Lo que GPUI carga por ruta: los íconos de Tabler en
/// `icons/tabler/<nombre>.svg`.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let Some(name) = tabler_name(path) else {
            return Ok(None);
        };
        let set = Tabler::get()?;
        Ok(set.svg(name).map(|svg| Cow::Owned(svg.into_bytes())))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        if !TABLER_DIR.starts_with(path) {
            return Ok(Vec::new());
        }
        let set = Tabler::get()?;
        Ok(set
            .names()
            .map(|name| format!("{TABLER_DIR}{name}{SVG_EXTENSION}").into())
            .collect())
    }
}

fn tabler_name(path: &str) -> Option<&str> {
    path.strip_prefix(TABLER_DIR)?.strip_suffix(SVG_EXTENSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_icono_de_la_ui_esta_en_el_set() {
        for icon in Icon::ALL {
            let loaded = Assets.load(icon.path()).unwrap();
            assert!(loaded.is_some(), "{icon:?}: {}", icon.path());
        }
    }

    #[test]
    fn carga_solo_rutas_de_tabler() {
        assert!(Assets.load("icons/tabler/folder.svg").unwrap().is_some());
        assert!(Assets.load("icons/tabler/no-existe.svg").unwrap().is_none());
        assert!(Assets.load("icons/tabler/folder").unwrap().is_none());
        assert!(Assets.load("icons/ph/folder.svg").unwrap().is_none());
        assert!(
            Assets
                .load("fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn lista_el_set_bajo_su_carpeta() {
        let all = Assets.list("icons/").unwrap();
        assert_eq!(all.len(), 6257);
        assert!(
            all.iter()
                .any(|path| path == "icons/tabler/folder-filled.svg")
        );
        assert_eq!(Assets.list("fonts/").unwrap(), Vec::<SharedString>::new());
    }
}
