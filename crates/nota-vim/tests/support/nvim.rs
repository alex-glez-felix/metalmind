use std::process::{Command, Stdio};
use std::sync::OnceLock;

use nota_buffer::{Buffer, ByteOffset};

use super::{State, parse, render};

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/oracle.lua");

/// La tabla se valida contra esta versión; el CI instala la misma.
const VERSION: &str = "NVIM v0.12.6";

/// Corre las mismas teclas en `nvim --clean` y devuelve el estado final.
pub fn run(before: &str, keys: &str) -> State {
    assert_eq!(
        installed_version(),
        VERSION,
        "el oráculo necesita {VERSION}; el `nvim` del PATH es otro"
    );
    let (buffer, selections) = parse(before);
    assert_eq!(selections.iter().len(), 1, "nvim no tiene multi-cursor");
    let head = selections.primary().head;
    let row = buffer.line_of(head);
    let col = head.get() - buffer.line_start(row).get();
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new("nvim")
        .args(["--clean", "--headless", "-n", "-l", SCRIPT])
        .arg(buffer.to_string())
        .arg((row + 1).to_string())
        .arg(col.to_string())
        .arg(keys)
        .arg(output.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "nvim terminó con {status}");
    let dump = std::fs::read_to_string(output.path()).unwrap();
    let (header, text) = dump.split_once('\n').unwrap_or((&dump, ""));
    let mut fields = header.split(' ');
    let mut number = || fields.next().unwrap().parse::<usize>().unwrap();
    let (row, col) = (number(), number());
    let mode = fields.next().unwrap().to_owned();
    let result = Buffer::from(text);
    let head = ByteOffset::new(result.line_start(row - 1).get() + col);
    State {
        text: render(&result, [head]),
        mode,
    }
}

fn installed_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let output = Command::new("nvim").arg("--version").output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        stdout.lines().next().unwrap_or_default().to_owned()
    })
}
