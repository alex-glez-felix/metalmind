//! Mide un motor en un proceso nuevo, para que la primera fórmula sea de
//! verdad la primera:
//!
//! `medir <typst|ratex|mathml|mathcore> [escala]`
//!
//! Imprime una fila por fórmula y una línea `resumen` que junta
//! `medir.sh` con las de varias corridas.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use metalmind_spike_math::formulas::{FORMULAS, Formula};
use metalmind_spike_math::{Engine, Kind, Look, MathError, Phases, memory};
use peak_alloc::PeakAlloc;

/// El heap vivo: el RSS no baja cuando se libera memoria.
#[global_allocator]
static HEAP: PeakAlloc = PeakAlloc;

/// Fórmulas distintas para ver cómo crece la caché de Typst (`comemo`).
const UNIQUE: usize = 200;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(kind) = args.next().as_deref().and_then(Kind::parse) else {
        eprintln!("uso: medir <typst|ratex|mathml|mathcore> [escala]");
        return ExitCode::FAILURE;
    };
    let scale = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    match run(
        kind,
        Look {
            scale,
            ..Look::default()
        },
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}: {error}", kind.name());
            ExitCode::FAILURE
        }
    }
}

fn run(kind: Kind, look: Look) -> Result<(), MathError> {
    let before = sample();
    let start = Instant::now();
    let mut engine = kind.start()?;
    let started = start.elapsed();
    let after_start = sample();

    let [first, rest @ ..] = &FORMULAS;
    let first_time = render(engine.as_mut(), first, &look).map(|(phases, _)| phases.total());
    let after_first = sample();

    let mut fresh = Vec::new();
    let mut failed = Vec::new();
    let mut pixels = 0;
    for formula in rest {
        match render(engine.as_mut(), formula, &look) {
            Ok((phases, bytes)) => {
                print_row("nueva", formula, &phases);
                fresh.push(phases);
                pixels += bytes;
            }
            Err(error) => {
                println!("falla\t{}\t{error}", formula.name);
                failed.push(formula.name);
            }
        }
    }
    let after_fresh = sample();

    let again: Vec<Phases> = (0..5)
        .flat_map(|_| rest.iter())
        .filter_map(|formula| {
            render(engine.as_mut(), formula, &look)
                .ok()
                .map(|(phases, _)| phases)
        })
        .collect();

    let growth = unique_formulas(engine.as_mut(), &look);
    let after_unique = sample();
    comemo::evict(0);
    let after_evict = sample();

    println!(
        "resumen\tmotor={}\tescala={}\tarranque_ms={:.2}\tprimera_ms={}\tnueva_mediana_ms={:.3}\tnueva_p90_ms={:.3}\tnueva_max_ms={:.3}\t\
         convertir_ms={:.3}\tcomponer_ms={:.3}\tpintar_ms={:.3}\totra_vez_mediana_ms={:.3}\tunicas_ms={:.3}\t\
         rss_inicio_kib={}\trss_arranque_kib={}\trss_primera_kib={}\trss_22_kib={}\trss_unicas_kib={}\tpico_rss_kib={}\t\
         heap_arranque_kib={}\theap_primera_kib={}\theap_22_kib={}\theap_unicas_kib={}\theap_tras_evict_kib={}\tpico_heap_kib={}\t\
         imagenes_kib={}\tfallan={}",
        kind.name(),
        look.scale,
        ms(started),
        first_time.map_or_else(
            |error| format!("error: {error}"),
            |time| format!("{:.2}", ms(time))
        ),
        ms(percentile(fresh.iter().map(Phases::total), 50)),
        ms(percentile(fresh.iter().map(Phases::total), 90)),
        ms(percentile(fresh.iter().map(Phases::total), 100)),
        ms(percentile(fresh.iter().map(|p| p.convert), 50)),
        ms(percentile(fresh.iter().map(|p| p.layout), 50)),
        ms(percentile(fresh.iter().map(|p| p.raster), 50)),
        ms(percentile(again.iter().map(Phases::total), 50)),
        ms(growth),
        before.rss,
        after_start.rss,
        after_first.rss,
        after_fresh.rss,
        after_unique.rss,
        after_evict.peak_rss,
        after_start.heap,
        after_first.heap,
        after_fresh.heap,
        after_unique.heap,
        after_evict.heap,
        after_evict.peak_heap,
        pixels / 1024,
        failed.join(", "),
    );
    Ok(())
}

struct Sample {
    rss: u64,
    peak_rss: u64,
    heap: usize,
    peak_heap: usize,
}

/// Memoria en KiB: residente (lo que ve el sistema) y heap vivo.
fn sample() -> Sample {
    let process = memory::now();
    Sample {
        rss: process.rss,
        peak_rss: process.peak,
        heap: HEAP.current_usage() / 1024,
        peak_heap: HEAP.peak_usage() / 1024,
    }
}

/// Pinta y devuelve los tiempos y lo que ocupa la imagen.
fn render(
    engine: &mut dyn Engine,
    formula: &Formula,
    look: &Look,
) -> Result<(Phases, usize), MathError> {
    engine
        .render(formula.latex, formula.mode, look)
        .map(|rendered| (rendered.phases, rendered.image.bytes()))
}

/// Mediana del tiempo de `UNIQUE` fórmulas que nunca se vieron: las de la
/// lista con un término distinto al final.
fn unique_formulas(engine: &mut dyn Engine, look: &Look) -> Duration {
    let times = FORMULAS
        .iter()
        .filter(|formula| !formula.latex.starts_with(r"\begin"))
        .cycle()
        .take(UNIQUE)
        .enumerate()
        .filter_map(|(index, formula)| {
            let latex = format!("{} + {index}", formula.latex);
            engine
                .render(&latex, formula.mode, look)
                .ok()
                .map(|r| r.phases.total())
        });
    percentile(times, 50)
}

fn print_row(label: &str, formula: &Formula, phases: &Phases) {
    println!(
        "{label}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}",
        formula.name,
        ms(phases.convert),
        ms(phases.layout),
        ms(phases.raster),
        ms(phases.total()),
    );
}

fn percentile(times: impl Iterator<Item = Duration>, percent: usize) -> Duration {
    let mut times: Vec<Duration> = times.collect();
    times.sort_unstable();
    let Some(last) = times.len().checked_sub(1) else {
        return Duration::ZERO;
    };
    times[(last * percent).div_ceil(100)]
}

fn ms(time: Duration) -> f64 {
    time.as_secs_f64() * 1000.0
}
