//! Contar con callgrind las instrucciones de una medición, en un proceso
//! aparte: el mismo binario de tests, corriendo solo esa prueba.

use std::path::Path;
use std::process::{Command, Output};
use std::{env, fs, io};

/// Le dice al proceso hijo qué medición de su prueba contar y con qué tamaño.
const CHILD: &str = "METALMIND_CRECIMIENTO";

/// La función que callgrind delimita con un volcado antes y otro después. Con
/// el mismo patrón en las dos opciones, callgrind solo hace caso de una.
const BEFORE_MEASURED: &str = "*growth::measured*";
const AFTER_MEASURED: &str = "*metalmind_perf::growth::measured*";

#[derive(Debug, thiserror::Error)]
pub enum CountError {
    #[error(
        "una prueba de complejidad corre en el hilo de su test, con el nombre del test \
         (libtest o nextest)"
    )]
    NoTestName,
    #[error("no se pudo lanzar valgrind (ver docs/testing.md): {0}")]
    Valgrind(io::Error),
    #[error("callgrind con n = {size} terminó con {status}:\n{stderr}")]
    Failed {
        size: usize,
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("los volcados de callgrind: {0}")]
    Dumps(io::Error),
    #[error("callgrind no pasó por la función medida")]
    NotMeasured,
    #[error("la función medida corrió más de una vez")]
    MeasuredTwice,
    #[error("el volcado de callgrind no tiene el total de instrucciones")]
    NoSummary,
}

/// En el proceso hijo, qué medición contar y con qué tamaño.
pub fn child() -> Option<(usize, usize)> {
    let value = env::var(CHILD).ok()?;
    let (call, size) = value.split_once(':')?;
    Some((call.parse().ok()?, size.parse().ok()?))
}

/// Las instrucciones de la medición `call` de la prueba `test` con tamaño
/// `size`, de todos los hilos.
pub fn instructions(test: &str, call: usize, size: usize) -> Result<u64, CountError> {
    let dir = tempfile::tempdir().map_err(CountError::Dumps)?;
    let out = dir.path().join("callgrind.out");
    let exe = env::current_exe().map_err(CountError::Valgrind)?;
    let Output { status, stderr, .. } = Command::new("valgrind")
        .arg("--tool=callgrind")
        .arg(format!("--callgrind-out-file={}", out.display()))
        .arg(format!("--dump-before={BEFORE_MEASURED}"))
        .arg(format!("--dump-after={AFTER_MEASURED}"))
        .arg(exe)
        .args([test, "--exact", "--include-ignored", "--nocapture"])
        .args(["--test-threads", "1"])
        .env(CHILD, format!("{call}:{size}"))
        .output()
        .map_err(CountError::Valgrind)?;
    if !status.success() {
        return Err(CountError::Failed {
            size,
            status,
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    measured_part(dir.path())
}

/// Las instrucciones del volcado que cierra la función medida.
fn measured_part(dir: &Path) -> Result<u64, CountError> {
    let mut after = Vec::new();
    for entry in fs::read_dir(dir).map_err(CountError::Dumps)? {
        let part = fs::read_to_string(entry.map_err(CountError::Dumps)?.path())
            .map_err(CountError::Dumps)?;
        if part.contains("desc: Trigger: --dump-after") {
            after.push(part);
        }
    }
    let part = match after.as_slice() {
        [] => return Err(CountError::NotMeasured),
        [part] => part,
        _ => return Err(CountError::MeasuredTwice),
    };
    part.lines()
        .find_map(|line| line.strip_prefix("summary:"))
        .and_then(|count| count.trim().parse().ok())
        .ok_or(CountError::NoSummary)
}
