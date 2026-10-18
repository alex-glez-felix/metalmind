//! Lo que deja nvim en los paneles tras unas teclas: el oráculo de los
//! tests que cambian de panel o de archivo desde un modo a medias.

use std::path::Path;
use std::process::{Command, Stdio};

const SCRIPT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/workspace/panes_oracle.lua"
);

/// La misma que exige el oráculo de `nota-vim`; el CI instala esa.
const VERSION: &str = "NVIM v0.12.6";

/// Un panel: su archivo y el cursor, con la línea desde 1 y la columna en
/// bytes, como `nvim_win_get_cursor`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Window {
    pub file: String,
    pub focused: bool,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Panes {
    /// Como `mode()` de vim: `n`, `i`, `c`…
    pub mode: String,
    /// En orden, sin importar el lado: nota divide a la derecha y abajo
    /// (`splitright`, `splitbelow`) y nvim `--clean`, al revés.
    pub windows: Vec<Window>,
}

/// Abre `file` de `dir` en `nvim --clean` y teclea `keys`.
pub(crate) fn run(dir: &Path, file: &str, keys: &str) -> Panes {
    let version = Command::new("nvim").arg("--version").output().unwrap();
    let version = String::from_utf8(version.stdout).unwrap();
    assert_eq!(
        version.lines().next(),
        Some(VERSION),
        "el oráculo necesita {VERSION}; el `nvim` del PATH es otro"
    );
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new("nvim")
        .args(["--clean", "--headless", "-n", "-l", SCRIPT])
        .arg(dir)
        .arg(file)
        .arg(keys)
        .arg(output.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "nvim terminó con {status}");
    let dump = std::fs::read_to_string(output.path()).unwrap();
    let mut lines = dump.lines();
    let mode = lines.next().unwrap().to_owned();
    let mut windows: Vec<Window> = lines
        .map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            Window {
                file: fields[0].to_owned(),
                focused: fields[1] == "1",
                line: fields[2].parse().unwrap(),
                column: fields[3].parse().unwrap(),
            }
        })
        .collect();
    windows.sort();
    Panes { mode, windows }
}
