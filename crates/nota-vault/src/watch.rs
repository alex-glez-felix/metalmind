//! De los eventos de `notify` a [`Touched`]: qué rutas del vault releer.

use std::path::Path;

use notify::EventKind;
use notify::event::{ModifyKind, RenameMode};
use notify_debouncer_full::DebouncedEvent;

use crate::path::VaultPath;
use crate::refresh::Touched;

/// Los archivos que cambian qué se ignora: tocarlos relee su carpeta.
const IGNORE_FILES: [&str; 2] = [".gitignore", ".ignore"];

/// Las reglas de exclusión de git del vault: tocarlas relee todo.
const GIT_EXCLUDE: &str = ".git/info/exclude";

pub(crate) fn touched(root: &Path, events: &[DebouncedEvent]) -> Touched {
    let mut touched = Touched::default();
    for event in events {
        if event.need_rescan() {
            touched.paths.insert(VaultPath::root());
            continue;
        }
        if !affects_index(event.kind) {
            continue;
        }
        let paths: Vec<Option<VaultPath>> = event
            .paths
            .iter()
            .map(|path| relevant(root, path))
            .collect();
        if let (EventKind::Modify(ModifyKind::Name(RenameMode::Both)), [Some(from), Some(to)]) =
            (event.kind, paths.as_slice())
        {
            touched.renames.push((from.clone(), to.clone()));
        }
        touched.paths.extend(paths.into_iter().flatten());
    }
    touched
}

/// Tras un error del watcher pudo perderse algo: se relee lo que el error
/// nombra, con las mismas reglas que un evento, o todo el vault si no nombra
/// nada o nombra algo que no se puede ubicar.
pub(crate) fn touched_by_errors(root: &Path, errors: &[notify::Error]) -> Touched {
    let mut touched = Touched::default();
    for error in errors {
        if error.paths.is_empty() {
            touched.paths.insert(VaultPath::root());
        }
        touched.paths.extend(
            error
                .paths
                .iter()
                .map(|path| relevant(root, path).unwrap_or_else(VaultPath::root)),
        );
    }
    touched
}

/// Abrir, leer o cambiar permisos no cambia qué hay en el vault.
fn affects_index(kind: EventKind) -> bool {
    !matches!(
        kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_))
    )
}

/// La ruta del vault que hay que releer por un evento en `path`, si alguna.
fn relevant(root: &Path, path: &Path) -> Option<VaultPath> {
    let relative = path.strip_prefix(root).ok()?;
    if relative == Path::new(GIT_EXCLUDE) {
        return Some(VaultPath::root());
    }
    let name = relative.file_name().and_then(|name| name.to_str());
    if let Some(name) = name
        && IGNORE_FILES.contains(&name)
    {
        return VaultPath::from_fs(root, path.parent()?);
    }
    // Un oculto (temporal de un guardado, algo en `.git/`) no está en el índice.
    VaultPath::from_fs(root, path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Instant;

    use notify::Event;
    use notify::event::{AccessKind, CreateKind, DataChange, Flag, MetadataKind};
    use pretty_assertions::assert_eq;

    use super::*;

    fn event(kind: EventKind, paths: &[&str]) -> DebouncedEvent {
        let event = paths.iter().fold(Event::new(kind), |event, path| {
            event.add_path(PathBuf::from(path))
        });
        DebouncedEvent::new(event, Instant::now())
    }

    fn paths(touched: &Touched) -> Vec<&str> {
        touched.paths.iter().map(VaultPath::as_str).collect()
    }

    #[test]
    fn ignora_ocultos_y_lo_que_no_cambia_el_arbol() {
        let root = Path::new("/v");
        let events = [
            event(EventKind::Create(CreateKind::File), &["/v/.nota-x.tmp"]),
            event(EventKind::Create(CreateKind::File), &["/v/.git/index"]),
            event(EventKind::Access(AccessKind::Read), &["/v/a.md"]),
            event(
                EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
                &["/v/b.md"],
            ),
            event(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/v/c.md"],
            ),
            event(EventKind::Create(CreateKind::File), &["/fuera/d.md"]),
        ];
        assert_eq!(paths(&touched(root, &events)), ["c.md"]);
    }

    #[test]
    fn un_rename_completo_queda_emparejado() {
        let root = Path::new("/v");
        let events = [event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &["/v/a.md", "/v/sub/b.md"],
        )];
        let touched = touched(root, &events);
        assert_eq!(paths(&touched), ["a.md", "sub/b.md"]);
        assert_eq!(touched.renames.len(), 1);
    }

    #[test]
    fn tocar_un_gitignore_relee_su_carpeta() {
        let root = Path::new("/v");
        let events = [
            event(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/v/sub/.gitignore"],
            ),
            event(EventKind::Create(CreateKind::File), &["/v/.ignore"]),
        ];
        assert_eq!(paths(&touched(root, &events)), ["", "sub"]);
    }

    #[test]
    fn tocar_el_exclude_de_git_relee_todo() {
        let events = [
            event(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/v/.git/info/exclude"],
            ),
            event(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/v/.git/index"],
            ),
        ];
        assert_eq!(paths(&touched(Path::new("/v"), &events)), [""]);
    }

    #[test]
    fn un_error_relee_lo_que_nombra_con_las_reglas_de_los_eventos() {
        let root = Path::new("/v");
        let error = |paths: &[&str]| {
            paths
                .iter()
                .fold(notify::Error::generic("x"), |error, path| {
                    error.add_path(PathBuf::from(path))
                })
        };
        let only = |errors: &[notify::Error]| paths(&touched_by_errors(root, errors)).join(",");
        assert_eq!(only(&[error(&["/v/sub/a.md"])]), "sub/a.md");
        assert_eq!(only(&[error(&["/v/sub/.gitignore"])]), "sub");
        assert_eq!(only(&[error(&["/v/.git/index"])]), "");
        assert_eq!(only(&[error(&[])]), "");
    }

    #[test]
    fn rescan_relee_todo() {
        let event = Event::new(EventKind::Other).set_flag(Flag::Rescan);
        let events = [DebouncedEvent::new(event, Instant::now())];
        assert_eq!(paths(&touched(Path::new("/v"), &events)), [""]);
    }
}
