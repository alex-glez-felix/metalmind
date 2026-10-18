//! Un servidor Xvfb por test. `-displayfd` le deja elegir un display libre y
//! escribirlo en stdout, así que no hay carreras por el número.

use std::fs::File;
use std::io::{BufRead as _, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub struct Xvfb {
    child: Child,
    name: String,
}

impl Xvfb {
    pub fn start(log: &Path) -> Self {
        let log = File::create(log).expect("crear el log de Xvfb");
        let mut child = Command::new("Xvfb")
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
        let mut number = String::new();
        BufReader::new(stdout)
            .read_line(&mut number)
            .expect("leer el display de Xvfb");
        let number = number.trim();
        assert!(!number.is_empty(), "Xvfb terminó sin abrir un display");
        Self {
            name: format!(":{number}"),
            child,
        }
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
