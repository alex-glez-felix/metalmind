//! Lee el registro de `metalmind --frame-log` (`FrameClock` en `metalmind-ui`) y
//! resume los tiempos.

use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use crate::support::wait;

/// Un frame presentado; los tiempos en µs, como los escribe `metalmind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub t: u64,
    pub cpu: Option<u64>,
    pub input: Option<u64>,
    /// Las teclas que el frame muestra por primera vez.
    pub keys: usize,
}

/// Solo las líneas completas: la última puede estar a medio escribir.
pub fn read(log: &Path) -> Vec<Frame> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .split_inclusive('\n')
        .filter_map(|line| parse(line.strip_suffix('\n')?))
        .collect()
}

fn parse(line: &str) -> Option<Frame> {
    field::<u64>(line, "frame")?;
    Some(Frame {
        t: field(line, "t")?,
        cpu: field(line, "cpu"),
        input: field(line, "input"),
        keys: field(line, "keys").unwrap_or(0),
    })
}

fn field<T: FromStr>(line: &str, name: &str) -> Option<T> {
    line.split(' ')
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        .and_then(|value| value.parse().ok())
}

/// Cuántas teclas mostraron ya los frames del registro.
pub fn keys_shown(log: &Path) -> usize {
    read(log).iter().map(|frame| frame.keys).sum()
}

/// Espera el frame con el que los frames llegan a mostrar `total` teclas.
pub fn wait_for_keys(log: &Path, total: usize, timeout: Duration) -> Frame {
    wait::until(&format!("un frame con la tecla {total}"), timeout, || {
        let mut shown = 0;
        for frame in read(log) {
            shown += frame.keys;
            if shown >= total {
                return Ok(frame);
            }
        }
        Err(format!("{shown} teclas mostradas"))
    })
}

/// Espera a que pasen `quiet` sin frames nuevos y devuelve el último: lo
/// que llegó del fondo (las imágenes de las fórmulas) ya se pintó.
pub fn wait_settled(log: &Path, quiet: Duration, timeout: Duration) -> Frame {
    let mut last = None;
    let mut since = std::time::Instant::now();
    wait::until("que dejen de llegar frames", timeout, || {
        let now = read(log).last().copied();
        if now != last {
            last = now;
            since = std::time::Instant::now();
        }
        match last {
            Some(frame) if since.elapsed() >= quiet => Ok(frame),
            _ => Err(format!("el último frame es {last:?}")),
        }
    })
}

/// Espera el primer frame.
pub fn wait_first(log: &Path, timeout: Duration) -> Frame {
    wait::until("el primer frame", timeout, || {
        read(log)
            .first()
            .copied()
            .ok_or_else(|| "ningún frame".to_owned())
    })
}

/// p50, p95 y máximo, en ms.
pub struct Summary {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

impl Summary {
    pub fn of_micros(values: impl IntoIterator<Item = u64>) -> Self {
        let mut sorted: Vec<u64> = values.into_iter().collect();
        assert!(!sorted.is_empty(), "nada que resumir");
        sorted.sort_unstable();
        // Rango más cercano: el menor valor con al menos `percent` % de las muestras.
        let at = |percent: usize| millis(sorted[(percent * sorted.len()).div_ceil(100).max(1) - 1]);
        Self {
            p50: at(50),
            p95: at(95),
            max: at(100),
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "µs de un benchmark, muy lejos de 2^52"
)]
pub fn millis(micros: u64) -> f64 {
    micros as f64 / 1_000.
}
