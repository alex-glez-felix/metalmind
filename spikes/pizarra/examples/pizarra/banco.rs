//! El benchmark: mueve la cámara (y una caja) por un guion fijo, un paso por
//! frame, y mide cada frame. Como el de `spikes/gpui`: intervalo entre frames
//! y CPU del `render` al último `paint`.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use crate::pintar::Pintado;

pub const CALENTAR: usize = 30;
pub const POR_FASE: usize = 180;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fase {
    TodoALaVista,
    Zoom,
    PanAlCien,
    ArrastrarCaja,
}

impl Fase {
    pub const TODAS: [Self; 4] = [
        Self::TodoALaVista,
        Self::Zoom,
        Self::PanAlCien,
        Self::ArrastrarCaja,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Self::TodoALaVista => "toda la pizarra a la vista, pan lento",
            Self::Zoom => "zoom continuo de toda la pizarra a 3× y de vuelta",
            Self::PanAlCien => "pan al 100 %",
            Self::ArrastrarCaja => "arrastrar una caja con sus flechas, al 100 %",
        }
    }
}

/// En qué paso del guion va el frame `n` (contando desde el arranque).
pub fn paso(n: usize) -> Option<(Fase, f32)> {
    let n = n.checked_sub(CALENTAR)?;
    let fase = Fase::TODAS.get(n / POR_FASE)?;
    #[expect(clippy::cast_precision_loss, reason = "cientos de frames")]
    let t = (n % POR_FASE) as f32 / (POR_FASE - 1) as f32;
    Some((*fase, t))
}

#[derive(Debug, Default)]
struct Muestras {
    intervalos: Vec<Duration>,
    cpu: Vec<Duration>,
    elementos: usize,
    triangulos: usize,
    textos: usize,
}

#[derive(Debug, Default)]
pub struct Medidas {
    anterior: Option<Instant>,
    fase: Option<Fase>,
    por_fase: Vec<(Fase, Muestras)>,
    pub ultimo_cpu: Duration,
    pub ultimo_intervalo: Duration,
}

impl Medidas {
    pub fn empieza(&mut self, ahora: Instant, fase: Option<Fase>) {
        if let Some(antes) = self.anterior {
            self.ultimo_intervalo = ahora - antes;
            if let (Some(f), Some(actual)) = (fase, self.fase)
                && f == actual
                && let Some((_, m)) = self.por_fase.last_mut()
            {
                m.intervalos.push(self.ultimo_intervalo);
            }
        }
        if let Some(f) = fase
            && fase != self.fase
        {
            self.por_fase.push((f, Muestras::default()));
        }
        self.fase = fase;
        self.anterior = Some(ahora);
    }

    pub fn pintado(&mut self, empezo: Instant, pintado: Pintado) {
        self.ultimo_cpu = empezo.elapsed();
        if self.fase.is_some()
            && let Some((_, m)) = self.por_fase.last_mut()
        {
            m.cpu.push(self.ultimo_cpu);
            m.elementos = m.elementos.max(pintado.elementos);
            m.triangulos = m.triangulos.max(pintado.triangulos);
            m.textos = m.textos.max(pintado.textos);
        }
    }

    pub fn reporte(&self, titulo: &str) -> String {
        let mut out = format!("### {titulo}\n\n");
        out.push_str("| Fase | Intervalo p50 / p99 / máx. | CPU p50 / p99 / máx. | A la vista (máx.) | Triángulos | Textos |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for (fase, m) in &self.por_fase {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                fase.nombre(),
                percentiles(&m.intervalos),
                percentiles(&m.cpu),
                m.elementos,
                m.triangulos,
                m.textos
            );
        }
        out
    }
}

fn percentiles(muestras: &[Duration]) -> String {
    let mut orden = muestras.to_vec();
    orden.sort_unstable();
    let en = |p: usize| {
        let rango = (p * orden.len()).div_ceil(100).max(1);
        orden
            .get(rango - 1)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    };
    format!("{:.1} / {:.1} / {:.1} ms", en(50), en(99), en(100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_guion_calienta_y_luego_recorre_las_fases() {
        assert_eq!(paso(0), None);
        assert_eq!(paso(CALENTAR), Some((Fase::TodoALaVista, 0.0)));
        assert_eq!(
            paso(CALENTAR + POR_FASE - 1),
            Some((Fase::TodoALaVista, 1.0))
        );
        assert_eq!(paso(CALENTAR + POR_FASE).map(|p| p.0), Some(Fase::Zoom));
        assert_eq!(paso(CALENTAR + 4 * POR_FASE), None);
    }
}
