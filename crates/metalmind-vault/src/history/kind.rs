//! De dónde sale cada versión y cómo se agrupan las del autoguardado (06 §3).
//! Mientras se sigue editando, las versiones autoguardadas de un mismo tramo
//! de [`AUTOSAVE_GROUP`] se reemplazan entre sí y queda la última, así que
//! sale una cada pocos minutos y la última al dejar de editar. Un guardado
//! pedido y un cambio desde fuera siempre dejan la suya.

use std::time::Duration;

use super::log::Entry;
use super::{ContentHash, millis};

/// Cuánto dura un grupo de versiones del autoguardado, desde la primera.
pub const AUTOSAVE_GROUP: Duration = Duration::from_mins(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionKind {
    /// Un guardado que alguien pidió, como `:w`.
    Saved,
    Autosaved,
    /// Lo que cambió fuera de Metalmind y vio el watcher.
    External,
}

impl VersionKind {
    pub(super) fn tag(self) -> u8 {
        match self {
            Self::Saved => 0,
            Self::Autosaved => 1,
            Self::External => 2,
        }
    }

    pub(super) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Saved),
            1 => Some(Self::Autosaved),
            2 => Some(Self::External),
            _ => None,
        }
    }
}

/// Qué hace una versión nueva con la última del log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Placement {
    /// No hace falta: tiene lo mismo que la última.
    Skip,
    /// Va detrás de la última; su grupo empieza en `group`.
    After { group: i64 },
    /// Reemplaza a la última, que era autoguardada.
    Replaces { group: i64 },
}

/// Dónde va una versión de tipo `kind` con hash `hash`, guardada en `at`.
pub(super) fn place(
    last: Option<&Entry>,
    kind: VersionKind,
    hash: ContentHash,
    at: i64,
) -> Placement {
    let Some(last) = last else {
        return Placement::After { group: at };
    };
    let autosaved = last.kind == VersionKind::Autosaved;
    if last.hash == hash {
        // Un `:w` de lo que se acaba de autoguardar fija esa versión: la
        // siguiente autoguardada ya no la reemplaza.
        return if autosaved && kind == VersionKind::Saved {
            Placement::Replaces { group: at }
        } else {
            Placement::Skip
        };
    }
    // Con el reloj hacia atrás, otro grupo: reemplazar borraría una versión ajena.
    let same_group = (last.group..last.group.saturating_add(millis(AUTOSAVE_GROUP))).contains(&at);
    if kind == VersionKind::Autosaved && autosaved && same_group {
        Placement::Replaces { group: last.group }
    } else {
        Placement::After { group: at }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const MINUTE: i64 = 60_000;

    fn entry(kind: VersionKind, text: &str, at: i64, group: i64) -> Entry {
        Entry {
            seq: 0,
            at,
            hash: ContentHash::of(text),
            len: text.len(),
            record: 0,
            kind,
            group,
        }
    }

    fn hash(text: &str) -> ContentHash {
        ContentHash::of(text)
    }

    #[test]
    fn el_autoguardado_reemplaza_a_los_de_su_grupo() {
        let last = entry(VersionKind::Autosaved, "a", 2 * MINUTE, 0);
        let autosave = |at| place(Some(&last), VersionKind::Autosaved, hash("b"), at);
        assert_eq!(autosave(4 * MINUTE), Placement::Replaces { group: 0 });
        assert_eq!(autosave(5 * MINUTE), Placement::After { group: 5 * MINUTE });
        assert_eq!(
            autosave(-MINUTE),
            Placement::After { group: -MINUTE },
            "con el reloj hacia atrás empieza otro grupo"
        );
    }

    #[test]
    fn un_guardado_pedido_o_de_fuera_siempre_va_detras() {
        let last = entry(VersionKind::Autosaved, "a", MINUTE, 0);
        for kind in [VersionKind::Saved, VersionKind::External] {
            assert_eq!(
                place(Some(&last), kind, hash("b"), 2 * MINUTE),
                Placement::After { group: 2 * MINUTE }
            );
        }
    }

    #[test]
    fn tras_un_guardado_pedido_empieza_otro_grupo() {
        let last = entry(VersionKind::Saved, "a", MINUTE, MINUTE);
        assert_eq!(
            place(Some(&last), VersionKind::Autosaved, hash("b"), 2 * MINUTE),
            Placement::After { group: 2 * MINUTE }
        );
    }

    #[test]
    fn w_de_lo_ya_autoguardado_fija_la_version() {
        let last = entry(VersionKind::Autosaved, "a", MINUTE, 0);
        assert_eq!(
            place(Some(&last), VersionKind::Saved, hash("a"), 2 * MINUTE),
            Placement::Replaces { group: 2 * MINUTE }
        );
        for kind in [VersionKind::Autosaved, VersionKind::External] {
            assert_eq!(
                place(Some(&last), kind, hash("a"), 2 * MINUTE),
                Placement::Skip
            );
        }
        let saved = entry(VersionKind::Saved, "a", MINUTE, MINUTE);
        assert_eq!(
            place(Some(&saved), VersionKind::Saved, hash("a"), 2 * MINUTE),
            Placement::Skip
        );
    }
}
