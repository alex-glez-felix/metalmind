//! Contar con callgrind las instrucciones de una medición, en un proceso
//! aparte: el mismo binario de tests, corriendo solo esa prueba.

use std::ffi::CStr;
use std::path::Path;
use std::process::{Command, Output};
use std::{env, fs, io};

use valgrind_requests::callgrind;

/// Le dice al proceso hijo qué medición de su prueba contar y con qué tamaño.
const CHILD: &str = "METALMIND_CRECIMIENTO";

/// Con qué motivo se escribe el volcado de lo medido.
const MEASURED: &CStr = c"metalmind_perf::growth::measure";

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
    /// Con lo que dijo valgrind, que es lo único que dice por qué.
    #[error("el proceso de callgrind no llegó a la medición; valgrind dijo:\n{stderr}")]
    NotMeasured { stderr: String },
    #[error("el volcado de callgrind no tiene el total de instrucciones")]
    NoSummary,
}

/// En el proceso hijo, qué medición contar y con qué tamaño.
pub fn child() -> Option<(usize, usize)> {
    let value = env::var(CHILD).ok()?;
    let (call, size) = value.split_once(':')?;
    Some((call.parse().ok()?, size.parse().ok()?))
}

/// En el proceso hijo, corre `work` y deja sus instrucciones en un volcado
/// propio. Son client requests y no opciones con el nombre de una función:
/// callgrind las reconoce aunque no entienda los símbolos del binario.
pub fn measure<R>(work: impl FnOnce() -> R) -> R {
    callgrind::dump_stats();
    let output = work();
    callgrind::dump_stats_at(MEASURED);
    output
}

/// Las instrucciones de la medición `call` de la prueba `test` con tamaño
/// `size`, de todos los hilos.
pub fn instructions(test: &str, call: usize, size: usize) -> Result<u64, CountError> {
    let dir = tempfile::tempdir().map_err(CountError::Dumps)?;
    let dumps = dir.path().join("callgrind");
    // El hijo sale sin soltar lo medido: lo temporal que arme va aquí, aparte
    // de los volcados, y se borra con `dir`.
    let scratch = dir.path().join("tmp");
    for folder in [&dumps, &scratch] {
        fs::create_dir(folder).map_err(CountError::Dumps)?;
    }
    let out = dumps.join("callgrind.out");
    let exe = env::current_exe().map_err(CountError::Valgrind)?;
    let Output { status, stderr, .. } = Command::new("valgrind")
        .arg("--tool=callgrind")
        .arg(format!("--callgrind-out-file={}", out.display()))
        .arg(exe)
        .args([test, "--exact", "--include-ignored", "--nocapture"])
        .args(["--test-threads", "1"])
        .env(CHILD, format!("{call}:{size}"))
        .env("TMPDIR", &scratch)
        .output()
        .map_err(CountError::Valgrind)?;
    let stderr = String::from_utf8_lossy(&stderr).into_owned();
    if !status.success() {
        return Err(CountError::Failed {
            size,
            status,
            stderr,
        });
    }
    measured_part(&dumps)?.ok_or(CountError::NotMeasured { stderr })
}

/// Las instrucciones del volcado de lo medido; `None` si ninguno lo es.
fn measured_part(dir: &Path) -> Result<Option<u64>, CountError> {
    for entry in fs::read_dir(dir).map_err(CountError::Dumps)? {
        let part = fs::read_to_string(entry.map_err(CountError::Dumps)?.path())
            .map_err(CountError::Dumps)?;
        if part.lines().any(is_measured_trigger) {
            return part
                .lines()
                .find_map(|line| line.strip_prefix("summary:"))
                .and_then(|count| count.trim().parse().ok())
                .map(Some)
                .ok_or(CountError::NoSummary);
        }
    }
    Ok(None)
}

fn is_measured_trigger(line: &str) -> bool {
    line.strip_prefix("desc: Trigger: Client Request: ")
        .is_some_and(|reason| reason.as_bytes() == MEASURED.to_bytes())
}
