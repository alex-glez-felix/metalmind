//! La ventana: lo único del binario que toca GPUI. Preparar (config, vault y
//! documento) va aparte de abrir la ventana, para que `metalmind open --json`
//! anuncie la nota solo cuando ya no puede fallar al leerla.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{App, PathPromptOptions};
use metalmind_config::{Config, Diagnostic, HistoryRetention, Paths, Severity};
use metalmind_ui::{Document, FrameClock, OpenError, Startup};
use metalmind_vault::{
    FileError, HistoryOptions, Retention, Vault, VaultError, VaultOptions, VaultRoot,
};

use crate::target::{self, Sources, Target};

/// Lo que impide abrir la ventana.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("{0}")]
    Config(Diagnostic),
    #[error(transparent)]
    Target(#[from] target::TargetError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error("no se pudo entrar en el vault: {0}")]
    EnterVault(io::Error),
    #[error("no se pudo crear el registro de frames: {0}")]
    FrameLog(io::Error),
    #[error(transparent)]
    Window(OpenError),
    #[error(
        "no se pudo abrir el selector de carpetas: {0:#}\nPasa el vault como argumento \
         (`metalmind ~/Notas`) o ponlo en `vault:` de config.yaml"
    )]
    Chooser(#[source] anyhow::Error),
}

/// Las rutas de Metalmind y la config, lo primero que se lee.
pub struct Session {
    paths: Option<Paths>,
    config: Config,
}

impl Session {
    /// # Errors
    /// Solo si el tema embebido por defecto no carga.
    pub fn load() -> Result<Self, StartError> {
        let paths = Paths::detect()
            .inspect_err(|error| log::error!("{error}; se usa la configuración por defecto"))
            .ok();
        let config = load_config(paths.as_ref()).map_err(StartError::Config)?;
        Ok(Self { paths, config })
    }

    /// De dónde sale el vault si no se nombra uno.
    pub fn sources(&self) -> Sources {
        Sources::detect(self.paths.as_ref(), &self.config.settings)
    }
}

/// Lo que hace falta para abrir la ventana, ya leído.
pub struct Prepared {
    startup: Startup,
    config: Config,
}

/// Abre el vault de `target`, con su historial si hay directorio de datos,
/// y su archivo, si tiene; un archivo que no existe empieza vacío.
///
/// # Errors
/// Si el vault no se puede abrir o el archivo no se puede leer.
pub fn prepare(session: Session, target: &Target) -> Result<Prepared, StartError> {
    let Session { paths, config } = session;
    let root = VaultRoot::new(&target.vault)?;
    let options = VaultOptions {
        history: paths
            .as_ref()
            .map(|paths| history(paths, &root, &config.settings.history)),
        ..VaultOptions::default()
    };
    let (vault, updates) = Vault::open_with(root.path(), options)?;
    let file = target
        .file
        .as_deref()
        .map(|file| target::in_vault(vault.root(), file));
    std::env::set_current_dir(vault.root()).map_err(StartError::EnterVault)?;
    let document = match &file {
        Some(file) => Document::open(file)?,
        None => Document::unnamed(),
    };
    let startup = Startup {
        vault,
        updates,
        document,
        paths,
    };
    Ok(Prepared { startup, config })
}

/// El historial del vault de `root`: en su carpeta de datos, que sale de su
/// ruta canónica (06 §8), con la retención de `config.yaml`.
fn history(paths: &Paths, root: &VaultRoot, retention: &HistoryRetention) -> HistoryOptions {
    HistoryOptions {
        dir: paths.vault_data(root.path()).join("history"),
        retention: Retention {
            keep_all: retention.keep_all(),
            hourly: retention.hourly(),
            daily: retention.daily(),
            deleted: retention.deleted(),
        },
    }
}

/// El reloj de los frames; con `frame_log`, escribe ahí los tiempos de
/// cada uno. Va antes de [`prepare`], que entra en el vault: una ruta
/// relativa es la del directorio desde donde se lanzó `metalmind`.
///
/// # Errors
/// Si no se puede crear `frame_log`.
pub fn frame_clock(launched: Instant, frame_log: Option<&Path>) -> Result<FrameClock, StartError> {
    FrameClock::new(launched, frame_log).map_err(StartError::FrameLog)
}

/// Abre la ventana y vuelve cuando se cierra.
///
/// # Errors
/// Si no hay sesión gráfica, fuente monoespaciada o la plataforma no puede
/// abrir la ventana.
pub fn run(prepared: Prepared, clock: FrameClock) -> Result<(), StartError> {
    run_app(move |cx, failure| failure.check(open(prepared, clock, cx), cx))
}

/// Pregunta con el selector de carpetas del sistema qué vault abrir y lo abre
/// como [`run`]; si se cancela, sale sin abrir nada.
///
/// # Errors
/// Los de [`run`] y [`prepare`], o si el escritorio no tiene selector.
pub fn run_choosing_vault(session: Session, clock: FrameClock) -> Result<(), StartError> {
    run_app(move |cx, failure| {
        if let Err(error) = metalmind_ui::require_display(cx) {
            return failure.check(Err(StartError::Window(error)), cx);
        }
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Abrir como vault".into()),
        });
        cx.spawn(async move |cx| {
            let answer = chosen.await.unwrap_or_else(|closed| Err(closed.into()));
            cx.update(|cx| match chosen_vault(answer) {
                Ok(Some(vault)) => {
                    let target = Target { vault, file: None };
                    let opened =
                        prepare(session, &target).and_then(|prepared| open(prepared, clock, cx));
                    failure.check(opened, cx);
                }
                Ok(None) => cx.quit(),
                Err(error) => failure.check(Err(error), cx),
            });
        })
        .detach();
    })
}

/// La carpeta que devolvió el selector; `None` si se canceló.
fn chosen_vault(
    answer: anyhow::Result<Option<Vec<PathBuf>>>,
) -> Result<Option<PathBuf>, StartError> {
    let paths = answer.map_err(StartError::Chooser)?;
    Ok(paths.and_then(|paths| paths.into_iter().next()))
}

fn open(prepared: Prepared, clock: FrameClock, cx: &mut App) -> Result<(), StartError> {
    let Prepared { startup, config } = prepared;
    metalmind_ui::open(startup, &config, clock, cx).map_err(StartError::Window)
}

/// Corre la app con `start` y devuelve el error que la cerró, si hubo.
fn run_app(start: impl FnOnce(&mut App, Failure) + 'static) -> Result<(), StartError> {
    let failure = Failure::default();
    let in_app = failure.clone();
    gpui_kit::application().run(move |cx| start(cx, in_app));
    failure.0.take().map_or(Ok(()), Err)
}

/// Dónde deja la app el error con que se cierra, para que [`run_app`] lo
/// devuelva.
#[derive(Clone, Default)]
struct Failure(Rc<RefCell<Option<StartError>>>);

impl Failure {
    fn check(&self, result: Result<(), StartError>, cx: &mut App) {
        if let Err(error) = result {
            *self.0.borrow_mut() = Some(error);
            quit_from_event_loop(cx);
        }
    }
}

/// En Linux, el bucle de eventos olvida un `quit()` pedido antes de arrancar (#31); una tarea
/// del executor de primer plano solo corre ya dentro del bucle.
fn quit_from_event_loop(cx: &mut App) {
    cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
}

/// La config del usuario; lo que esté mal se reporta y se usa el default.
/// Sin `paths` no hay de dónde leerla.
fn load_config(paths: Option<&Paths>) -> Result<Config, Diagnostic> {
    let Some(paths) = paths else {
        return Config::defaults();
    };
    let loaded = Config::load(paths)?;
    for diagnostic in &loaded.diagnostics {
        match diagnostic.severity {
            Severity::Warning => log::warn!("{diagnostic}"),
            Severity::Error => log::error!("{diagnostic}"),
        }
    }
    Ok(loaded.value)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    const DAY: Duration = Duration::from_hours(24);

    #[test]
    fn el_historial_va_en_la_carpeta_de_datos_de_la_ruta_canonica() {
        let (dir, data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let paths = Paths::under(data.path());
        let real = dir.path().join("notas");
        fs::create_dir(&real).unwrap();
        symlink(&real, dir.path().join("enlace")).unwrap();
        let retention = HistoryRetention::default();
        let dir_of = |root: &Path| history(&paths, &VaultRoot::new(root).unwrap(), &retention).dir;

        let expected = paths
            .vault_data(&real.canonicalize().unwrap())
            .join("history");
        assert_eq!(dir_of(&real), expected);
        assert_eq!(dir_of(&dir.path().join("enlace")), expected);
        let moved = dir.path().join("movidas");
        fs::rename(&real, &moved).unwrap();
        assert_ne!(dir_of(&moved), expected, "un vault movido empieza de cero");
    }

    #[test]
    fn la_retencion_de_la_config_pasa_a_duraciones() {
        let data = TempDir::new().unwrap();
        let root = VaultRoot::new(data.path()).unwrap();
        let retention = HistoryRetention {
            keep_all_hours: 6,
            hourly_days: 7,
            daily_days: Some(365),
            deleted_days: 30,
        };
        let options = history(&Paths::under(data.path()), &root, &retention);
        assert_eq!(
            options.retention,
            Retention {
                keep_all: Duration::from_hours(6),
                hourly: 7 * DAY,
                daily: Some(365 * DAY),
                deleted: 30 * DAY,
            }
        );
    }

    #[test]
    fn el_selector_da_la_carpeta_elegida_o_nada_si_se_cancela() {
        let notas = PathBuf::from("/notas");
        assert_eq!(
            chosen_vault(Ok(Some(vec![notas.clone()]))).unwrap(),
            Some(notas)
        );
        assert_eq!(chosen_vault(Ok(None)).unwrap(), None);
        assert_eq!(chosen_vault(Ok(Some(Vec::new()))).unwrap(), None);
        assert!(matches!(
            chosen_vault(Err(anyhow::anyhow!("sin portal"))),
            Err(StartError::Chooser(_))
        ));
    }
}
