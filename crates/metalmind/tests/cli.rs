#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

//! El binario sin ventana: no necesita X.

use std::io::Read as _;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn version() {
    let output = Command::new(env!("CARGO_BIN_EXE_metalmind"))
        .arg("--version")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let expected = format!("metalmind {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

/// Sin Wayland ni X11 no hay dónde abrir la ventana: sale con error en vez de
/// quedarse corriendo sin ella (#31).
#[test]
fn sin_display_termina_con_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_metalmind"))
        .arg("a.md")
        .current_dir(dir.path())
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("config"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = wait_exit(&mut child, Duration::from_secs(10));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.and_then(|status| status.code()), Some(1), "{stderr}");
    assert!(stderr.contains("no hay sesión gráfica"), "{stderr}");
    assert!(stderr.contains("WAYLAND_DISPLAY ni DISPLAY"), "{stderr}");
}

/// Cómo terminó `child`, o `None` si a los `timeout` seguía vivo (y se mata).
fn wait_exit(child: &mut std::process::Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    None
}
