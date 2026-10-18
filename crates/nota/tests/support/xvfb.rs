//! Un servidor Xvfb por test, en el primer display libre desde `:100`.
//!
//! No se deja elegir a `-displayfd`: en ese modo Xvfb no mira el lock y, en un
//! escritorio con `XWayland`, puede quedarse con el `:0` de la sesión.

use std::fs::File;
use std::io::{BufRead as _, BufReader};
use std::ops::Range;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Lejos de los displays de un escritorio; si dos tests piden el mismo, el
/// lock de Xvfb deja pasar a uno y el otro prueba el siguiente.
const DISPLAYS: Range<u32> = 100..300;

pub struct Xvfb {
    child: Child,
    name: String,
}

impl Xvfb {
    pub fn start(log: &Path) -> Self {
        DISPLAYS
            .clone()
            .find_map(|number| Self::try_start(number, log))
            .unwrap_or_else(|| {
                panic!(
                    "ningún display libre en {DISPLAYS:?}; el último error está en {}",
                    log.display()
                )
            })
    }

    /// `None` si el display está ocupado. Con un display explícito,
    /// `-displayfd` solo avisa: escribe el número cuando ya acepta conexiones.
    fn try_start(number: u32, log: &Path) -> Option<Self> {
        let log = File::create(log).expect("crear el log de Xvfb");
        let name = format!(":{number}");
        let mut child = Command::new("Xvfb")
            .arg(&name)
            .args([
                "-displayfd",
                "1",
                "-screen",
                "0",
                "1280x800x24",
                "-nolisten",
                "tcp",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()
            .expect("lanzar Xvfb; ¿está instalado? ver docs/testing.md");
        let stdout = child.stdout.take().expect("stdout de Xvfb");
        // Bloquea hasta que el servidor acepta conexiones, o hasta que muere.
        let mut ready = String::new();
        BufReader::new(stdout)
            .read_line(&mut ready)
            .expect("leer el aviso de Xvfb");
        if ready.trim().is_empty() {
            child.wait().expect("esperar a Xvfb");
            return None;
        }
        Some(Self { child, name })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
