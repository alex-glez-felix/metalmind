//! Tiempos de arranque y de frame, sin depender de la percepción.
//!
//! - Intervalo: entre el inicio de dos frames seguidos. Bajo scroll continuo
//!   debe acercarse a 1000 / Hz del monitor.
//! - CPU: del `render` de la raíz al último `paint`; no incluye el envío a la
//!   GPU ni el `present`.

use std::time::{Duration, Instant};

/// Una pausa más larga es reposo, no un frame lento.
pub const BURST_GAP: Duration = Duration::from_millis(250);
/// Muestras recientes para el overlay en vivo.
const LIVE_WINDOW: usize = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub count: usize,
    pub p50: Duration,
    pub p99: Duration,
    pub max: Duration,
}

impl Summary {
    pub fn of(samples: &[Duration]) -> Option<Self> {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        Some(Self {
            count: sorted.len(),
            p50: percentile(&sorted, 50)?,
            p99: percentile(&sorted, 99)?,
            max: *sorted.last()?,
        })
    }
}

/// Percentil por rango más cercano sobre muestras ya ordenadas.
fn percentile(sorted: &[Duration], percent: usize) -> Option<Duration> {
    let rank = (percent * sorted.len()).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

#[derive(Debug)]
pub struct Metrics {
    launched: Instant,
    load: Option<Duration>,
    first_frame: Option<Duration>,
    last_frame_start: Option<Instant>,
    intervals: Vec<Duration>,
    cpu: Vec<Duration>,
}

impl Metrics {
    pub fn new(launched: Instant) -> Self {
        Self {
            launched,
            load: None,
            first_frame: None,
            last_frame_start: None,
            intervals: Vec::new(),
            cpu: Vec::new(),
        }
    }

    pub fn record_load(&mut self, load: Duration) {
        self.load = Some(load);
    }

    /// Descarta los frames medidos, p. ej. los del arranque antes de un benchmark.
    pub fn reset_frames(&mut self) {
        self.intervals.clear();
        self.cpu.clear();
    }

    pub fn frame_started(&mut self, now: Instant) {
        if let Some(previous) = self.last_frame_start {
            let gap = now.duration_since(previous);
            if gap <= BURST_GAP {
                self.intervals.push(gap);
            }
        }
        self.last_frame_start = Some(now);
    }

    pub fn frame_painted(&mut self, started: Instant, now: Instant) {
        self.cpu.push(now.duration_since(started));
        self.first_frame
            .get_or_insert_with(|| now.duration_since(self.launched));
    }

    pub fn load(&self) -> Option<Duration> {
        self.load
    }

    pub fn first_frame(&self) -> Option<Duration> {
        self.first_frame
    }

    pub fn frames(&self) -> usize {
        self.cpu.len()
    }

    pub fn last_cpu(&self) -> Option<Duration> {
        self.cpu.last().copied()
    }

    pub fn intervals(&self) -> Option<Summary> {
        Summary::of(&self.intervals)
    }

    pub fn cpu(&self) -> Option<Summary> {
        Summary::of(&self.cpu)
    }

    pub fn live_intervals(&self) -> Option<Summary> {
        Summary::of(recent(&self.intervals))
    }

    pub fn live_cpu(&self) -> Option<Summary> {
        Summary::of(recent(&self.cpu))
    }
}

fn recent(samples: &[Duration]) -> &[Duration] {
    &samples[samples.len().saturating_sub(LIVE_WINDOW)..]
}

pub fn millis(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1_000.0)
}

/// Frames por segundo que implica un intervalo típico.
pub fn fps(interval: Duration) -> String {
    let secs = interval.as_secs_f64();
    if secs > 0.0 {
        format!("{:.0}", 1.0 / secs)
    } else {
        "∞".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(values: &[u64]) -> Vec<Duration> {
        values.iter().copied().map(Duration::from_millis).collect()
    }

    #[test]
    fn percentiles_por_rango_mas_cercano() {
        let samples: Vec<u64> = (1..=100).collect();
        let summary = Summary::of(&ms(&samples)).unwrap();
        assert_eq!(summary.count, 100);
        assert_eq!(summary.p50, Duration::from_millis(50));
        assert_eq!(summary.p99, Duration::from_millis(99));
        assert_eq!(summary.max, Duration::from_millis(100));
    }

    #[test]
    fn una_sola_muestra_es_todos_los_percentiles() {
        let summary = Summary::of(&ms(&[7])).unwrap();
        assert_eq!(
            (summary.p50, summary.p99, summary.max),
            (
                Duration::from_millis(7),
                Duration::from_millis(7),
                Duration::from_millis(7)
            )
        );
    }

    #[test]
    fn sin_muestras_no_hay_resumen() {
        assert_eq!(Summary::of(&[]), None);
    }

    #[test]
    fn las_pausas_no_cuentan_como_intervalo() {
        let start = Instant::now();
        let mut metrics = Metrics::new(start);
        for offset in [0, 7, 14, 1_000, 1_007] {
            metrics.frame_started(start + Duration::from_millis(offset));
        }
        assert_eq!(metrics.intervals, ms(&[7, 7, 7]));
    }

    #[test]
    fn el_primer_frame_se_mide_desde_el_arranque() {
        let launched = Instant::now();
        let mut metrics = Metrics::new(launched);
        let started = launched + Duration::from_millis(300);
        metrics.frame_painted(started, started + Duration::from_millis(4));
        metrics.frame_painted(started, started + Duration::from_millis(900));
        assert_eq!(metrics.first_frame(), Some(Duration::from_millis(304)));
        assert_eq!(metrics.cpu, ms(&[4, 900]));
    }

    #[test]
    fn formato_legible() {
        assert_eq!(millis(Duration::from_micros(6_944)), "6.94 ms");
        assert_eq!(fps(Duration::from_micros(6_944)), "144");
    }
}
