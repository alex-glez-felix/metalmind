//! Un servidor Xvfb propio por test. El smoke no puede correr sobre un display
//! real: el display sale solo de un Xvfb que lanzó este proceso, se comprueba
//! por el lock de X que es suyo y no el del entorno, y si Xvfb no arranca el
//! test aborta. `DISPLAY` solo se le pasa a `metalmind`; nunca se lee para usarlo.
//!
//! No se usa `-displayfd`: con él, Xvfb ni mira ni escribe el lock de X, y en
//! un escritorio con `XWayland` puede quedarse con el `:0` de la sesión.

use std::env;
use std::fs::{self, File};
use std::ops::Range;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use x11rb::rust_connection::RustConnection;

use super::wait::{self, TIMEOUT};

/// Lejos de los displays de un escritorio; si dos tests piden el mismo, el
/// lock de Xvfb deja pasar a uno y el otro prueba el siguiente.
const DISPLAYS: Range<u32> = 100..300;

/// Fija un solo display (`METALMIND_SMOKE_DISPLAY=97`); con él, los tests van de
/// uno en uno (`--test-threads 1`).
const DISPLAY_VAR: &str = "METALMIND_SMOKE_DISPLAY";

/// La pantalla del smoke, que no mira píxeles.
pub const SCREEN: &str = "1280x800";

pub struct Xvfb {
    child: Child,
    name: String,
}

/// La conexión que abrió el guardia, y la única del harness: si se cerrara,
/// el servidor se reiniciaría al quedarse sin clientes y rechazaría la siguiente.
pub struct Connection {
    pub conn: RustConnection,
    pub screen: usize,
}

impl Xvfb {
    /// `screen` es el tamaño de la pantalla, como `1280x800`.
    pub fn start(log: &Path, screen: &str) -> (Self, Connection) {
        let displays = displays(env::var(DISPLAY_VAR).ok().as_deref());
        Self::start_with("Xvfb", displays, log, screen)
    }

    fn start_with(
        program: &str,
        displays: Range<u32>,
        log: &Path,
        screen: &str,
    ) -> (Self, Connection) {
        displays
            .clone()
            .find_map(|number| Self::try_start(program, number, log, screen))
            .unwrap_or_else(|| {
                panic!(
                    "ningún display libre en {displays:?}; el último error está en {}",
                    log.display()
                )
            })
    }

    /// `None` si el display está ocupado: Xvfb ve un lock vivo y sale.
    fn try_start(
        program: &str,
        number: u32,
        log: &Path,
        screen: &str,
    ) -> Option<(Self, Connection)> {
        let log = File::create(log).expect("crear el log de Xvfb");
        let name = format!(":{number}");
        let child = Command::new(program)
            .arg(&name)
            .args(["-screen", "0", &format!("{screen}x24"), "-nolisten", "tcp"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .unwrap_or_else(|error| {
                panic!(
                    "sin Xvfb el smoke no corre ({program}: {error}); nunca cae al \
                     DISPLAY del entorno. Ver docs/testing.md"
                )
            });
        let mut xvfb = Self { child, name };
        let connection = xvfb.wait_ready(number)?;
        Some((xvfb, connection))
    }

    /// El guardia: solo se conecta cuando el lock de X tiene el pid de este
    /// Xvfb, y nunca al display del entorno. Si falla, `Drop` mata al Xvfb.
    fn wait_ready(&mut self, number: u32) -> Option<Connection> {
        let session = env::var("DISPLAY").ok();
        assert_ne!(
            session.as_deref().and_then(display_number),
            Some(number),
            "{} es el DISPLAY del entorno; no se teclea ahí",
            self.name
        );
        let lock = format!("/tmp/.X{number}-lock");
        wait::until(
            &format!("que arranque Xvfb en {}", self.name),
            TIMEOUT,
            || {
                if self.child.try_wait().expect("consultar a Xvfb").is_some() {
                    return Ok(None);
                }
                let owner = fs::read_to_string(&lock)
                    .ok()
                    .and_then(|lock| lock_pid(&lock));
                if owner != Some(self.child.id()) {
                    return Err(format!("{lock} es de {owner:?}"));
                }
                x11rb::connect(Some(&self.name))
                    .map(|(conn, screen)| Some(Connection { conn, screen }))
                    .map_err(|error| error.to_string())
            },
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Xvfb {
    /// Con SIGTERM, Xvfb borra su socket y su lock; con SIGKILL quedarían.
    fn drop(&mut self) {
        let terminated = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .is_ok_and(|status| status.success());
        if !terminated {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn displays(fixed: Option<&str>) -> Range<u32> {
    match fixed {
        None => DISPLAYS,
        Some(number) => {
            let number: u32 = number
                .parse()
                .unwrap_or_else(|_| panic!("{DISPLAY_VAR}={number:?} no es un número"));
            number..number + 1
        }
    }
}

/// El lock de X guarda el pid del servidor en texto, con espacios delante.
fn lock_pid(lock: &str) -> Option<u32> {
    lock.trim().parse().ok()
}

/// El número de `:0`, `:0.0`, `unix:12.1` o `host:10`.
fn display_number(display: &str) -> Option<u32> {
    let (_, rest) = display.rsplit_once(':')?;
    rest.split('.').next()?.parse().ok()
}

#[test]
fn el_pid_del_lock_de_x() {
    assert_eq!(lock_pid("      1234\n"), Some(1234));
    assert_eq!(lock_pid("basura"), None);
}

#[test]
fn el_numero_de_un_display() {
    let cases = [
        (":0", Some(0)),
        (":0.0", Some(0)),
        ("unix:12.1", Some(12)),
        ("localhost:10", Some(10)),
        ("", None),
        ("wayland-1", None),
    ];
    for (display, expected) in cases {
        assert_eq!(display_number(display), expected, "{display:?}");
    }
}

#[test]
fn el_display_fijo_es_uno_solo() {
    assert_eq!(displays(None), DISPLAYS);
    assert_eq!(displays(Some("97")), 97..98);
}

#[test]
#[should_panic(expected = "no es un número")]
fn un_display_fijo_invalido_falla() {
    displays(Some(":97"));
}

#[test]
#[should_panic(expected = "sin Xvfb el smoke no corre")]
fn sin_xvfb_aborta_en_vez_de_usar_el_display_del_entorno() {
    let dir = tempfile::tempdir().unwrap();
    Xvfb::start_with(
        "/no/existe/Xvfb",
        100..101,
        &dir.path().join("xvfb.log"),
        SCREEN,
    );
}

#[test]
#[ignore = "necesita Xvfb: ver docs/testing.md"]
#[should_panic(expected = "ningún display libre")]
fn un_display_ocupado_no_se_comparte() {
    let dir = tempfile::tempdir().unwrap();
    let (first, _connection) = Xvfb::start(&dir.path().join("primero.log"), SCREEN);
    let number: u32 = first.name()[1..].parse().unwrap();
    Xvfb::start_with(
        "Xvfb",
        number..number + 1,
        &dir.path().join("segundo.log"),
        SCREEN,
    );
}
