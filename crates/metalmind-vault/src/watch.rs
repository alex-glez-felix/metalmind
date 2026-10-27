//! De los eventos de `notify` a [`Touched`]: qué rutas del vault releer.

use std::collections::HashSet;
use std::path::Path;

use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind};

use crate::path::VaultPath;
use crate::refresh::Touched;
use crate::scan::ruled_by;

pub(crate) fn touched(root: &Path, events: &[Event]) -> Touched {
    let paired: HashSet<usize> = events
        .iter()
        .filter(|event| event.kind == EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
        .filter_map(|event| event.attrs.tracker())
        .collect();
    let mut touched = Touched::default();
    for event in events {
        if event.need_rescan() {
            touched.paths.insert(VaultPath::root());
            continue;
        }
        let named: Vec<Option<VaultPath>> = event
            .paths
            .iter()
            .map(|path| VaultPath::from_fs(root, path))
            .collect();
        match (event.kind, named.as_slice()) {
            (EventKind::Modify(ModifyKind::Name(RenameMode::Both)), [Some(from), Some(to)]) => {
                touched.renames.push((from.clone(), to.clone()));
            }
            // Movido desde lo que el vault no ve (el temporal oculto de un
            // guardado atómico, otra carpeta): el destino tiene contenido nuevo.
            (EventKind::Modify(ModifyKind::Name(RenameMode::Both)), [None, Some(to)]) => {
                touched.written.insert(to.clone());
            }
            (EventKind::Modify(ModifyKind::Name(RenameMode::To)), [Some(to)])
                if event
                    .attrs
                    .tracker()
                    .is_none_or(|tracker| !paired.contains(&tracker)) =>
            {
                touched.written.insert(to.clone());
            }
            _ => {}
        }
        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any)
        ) {
            touched.written.extend(named.iter().flatten().cloned());
        }
        // Un archivo de reglas también relee la parte del vault que decide.
        let ruled = event.paths.iter().filter_map(|path| ruled_by(root, path));
        let paths = named.into_iter().flatten().chain(ruled);
        if matches!(event.kind, EventKind::Modify(ModifyKind::Metadata(_))) {
            touched.reread.extend(paths);
        } else {
            touched.paths.extend(paths);
        }
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
        touched.paths.extend(error.paths.iter().map(|path| {
            ruled_by(root, path)
                .or_else(|| VaultPath::from_fs(root, path))
                .unwrap_or_else(VaultPath::root)
        }));
    }
    touched
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use notify::event::{CreateKind, DataChange, Flag, MetadataKind};
    use pretty_assertions::assert_eq;

    use super::*;

    fn event(kind: EventKind, paths: &[&str]) -> Event {
        paths.iter().fold(Event::new(kind), |event, path| {
            event.add_path(PathBuf::from(path))
        })
    }

    fn paths(touched: &Touched) -> Vec<&str> {
        touched.paths.iter().map(VaultPath::as_str).collect()
    }

    #[test]
    fn ignora_ocultos_y_lo_que_no_cambia_el_contenido() {
        let root = Path::new("/v");
        let events = [
            event(
                EventKind::Create(CreateKind::File),
                &["/v/.metalmind-x.tmp"],
            ),
            event(EventKind::Create(CreateKind::File), &["/v/.git/index"]),
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
        let touched = touched(root, &events);
        assert_eq!(paths(&touched), ["c.md"]);
        // Un chmod puede volver legible una carpeta: se relee, sin ser una modificación.
        let reread: Vec<_> = touched.reread.iter().map(VaultPath::as_str).collect();
        assert_eq!(reread, ["b.md"]);
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

    fn renamed(mode: RenameMode, paths: &[&str], cookie: usize) -> Event {
        event(EventKind::Modify(ModifyKind::Name(mode)), paths).set_tracker(cookie)
    }

    fn written(touched: &Touched) -> Vec<&str> {
        touched.written.iter().map(VaultPath::as_str).collect()
    }

    #[test]
    fn lo_movido_desde_lo_que_el_vault_no_ve_es_contenido_nuevo() {
        let root = Path::new("/v");
        // El guardado atómico: un temporal oculto renombrado encima.
        let atomic = [
            renamed(RenameMode::From, &["/v/.tmp"], 1),
            renamed(RenameMode::To, &["/v/a.md"], 1),
            renamed(RenameMode::Both, &["/v/.tmp", "/v/a.md"], 1),
        ];
        assert_eq!(written(&touched(root, &atomic)), ["a.md"]);
        // Desde fuera del vault no hay `From`, ni par.
        let moved_in = [renamed(RenameMode::To, &["/v/b.md"], 2)];
        assert_eq!(written(&touched(root, &moved_in)), ["b.md"]);
        // Un rename dentro del vault mueve el contenido, no lo escribe.
        let inside = [
            renamed(RenameMode::From, &["/v/a.md"], 3),
            renamed(RenameMode::To, &["/v/b.md"], 3),
            renamed(RenameMode::Both, &["/v/a.md", "/v/b.md"], 3),
        ];
        let touched = touched(root, &inside);
        assert_eq!(written(&touched), Vec::<&str>::new());
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

    /// Si no, la raíz quedaría como destino de un rename y todo lo que hay
    /// dentro se daría por modificado.
    #[test]
    fn renombrar_un_archivo_de_reglas_no_es_un_rename_del_vault() {
        let root = Path::new("/v");
        let events = [event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &["/v/.gitignore", "/v/.ignore"],
        )];
        let touched = touched(root, &events);
        assert_eq!(paths(&touched), [""]);
        assert_eq!(touched.renames, []);
    }

    /// Una carpeta que se llama como un archivo de reglas sigue siendo del
    /// vault: su rename se anota.
    #[test]
    fn renombrar_una_carpeta_llamada_como_reglas_es_un_rename() {
        let root = Path::new("/v");
        let events = [event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &["/v/CACHEDIR.TAG", "/v/otra"],
        )];
        let renames = touched(root, &events).renames;
        let pairs: Vec<(&str, &str)> = renames
            .iter()
            .map(|(from, to)| (from.as_str(), to.as_str()))
            .collect();
        assert_eq!(pairs, [("CACHEDIR.TAG", "otra")]);
    }

    #[test]
    fn marcar_una_carpeta_como_cache_la_relee() {
        let root = Path::new("/v");
        let events = [event(
            EventKind::Create(CreateKind::File),
            &["/v/sub/target/CACHEDIR.TAG"],
        )];
        assert_eq!(
            paths(&touched(root, &events)),
            ["sub/target", "sub/target/CACHEDIR.TAG"]
        );
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
        let events = [event];
        assert_eq!(paths(&touched(Path::new("/v"), &events)), [""]);
    }
}
