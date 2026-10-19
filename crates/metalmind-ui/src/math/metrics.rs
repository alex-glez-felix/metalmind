//! El layout de las fórmulas, en caché y en el hilo de la UI: lo piden el
//! motor, para saber cuánto mide cada línea al mover la ventana, y la
//! vista, para pintar. Medir una fórmula nueva cuesta unos µs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use metalmind_syntax::MathSpot;
use metalmind_vim::FormulaRows;

use super::engine::{self, MathError, Metrics};
use super::geometry::Geometry;

/// Cuántas fórmulas guarda cada generación de la caché.
const GENERATION: usize = 4096;

type Measured = Result<Metrics, MathError>;

/// Lo medido por LaTeX, en dos generaciones: cuando la nueva se llena, la
/// vieja se tira y lo que se sigue usando pasa a la nueva. Así no crece sin
/// tope mientras se escribe dentro de un bloque, y lo visible no se mide
/// dos veces.
#[derive(Debug, Default)]
pub struct MetricsCache {
    generations: Mutex<Generations>,
}

#[derive(Debug, Default)]
struct Generations {
    /// Por `display`: `$…$` en la 0 y `$$…$$` en la 1.
    current: [HashMap<String, Measured>; 2],
    previous: [HashMap<String, Measured>; 2],
}

impl MetricsCache {
    /// Lo que mide la fórmula, de la caché o medida ahora.
    ///
    /// # Errors
    /// Si RaTeX no la entiende o no puede componerla.
    pub fn get(&self, latex: &str, display: bool) -> Measured {
        let mut generations = self
            .generations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let kind = usize::from(display);
        if let Some(found) = generations.current[kind].get(latex) {
            return found.clone();
        }
        let found = generations.previous[kind]
            .remove(latex)
            .unwrap_or_else(|| engine::measure(latex, display));
        if generations.current[kind].len() >= GENERATION {
            generations.previous[kind] = std::mem::take(&mut generations.current[kind]);
        }
        generations.current[kind].insert(latex.to_owned(), found.clone());
        found
    }

    /// La fórmula se midió pero no se pudo pintar: se trata como una que
    /// no compone.
    pub fn fail(&self, latex: &str, display: bool, error: MathError) {
        let mut generations = self
            .generations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let kind = usize::from(display);
        generations.previous[kind].remove(latex);
        generations.current[kind].insert(latex.to_owned(), Err(error));
    }
}

/// Lo que el motor le pide a la vista: cuánto mide la línea de cada
/// fórmula con la letra de este momento. Uno nuevo por cada letra distinta,
/// que comparten la caché.
#[derive(Debug)]
pub struct FormulaMeasure {
    cache: Arc<MetricsCache>,
    geometry: Geometry,
}

impl FormulaMeasure {
    pub fn new(cache: Arc<MetricsCache>, geometry: Geometry) -> Self {
        Self { cache, geometry }
    }

    #[must_use]
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }
}

impl FormulaRows for FormulaMeasure {
    fn rows(&self, formula: &MathSpot) -> Option<usize> {
        let metrics = self.cache.get(&formula.latex, formula.display).ok()?;
        Some(
            self.geometry
                .rows(self.geometry.formula(metrics), formula.place),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mide_una_vez_y_recuerda_los_errores() {
        let cache = MetricsCache::default();
        let first = cache.get("x^2", false);
        assert!(first.is_ok());
        assert_eq!(cache.get("x^2", false), first);
        assert!(cache.get(r"\frac{", false).is_err());
        assert!(cache.get(r"\frac{", false).is_err());
    }

    #[test]
    fn una_que_no_se_pudo_pintar_queda_como_error() {
        let cache = MetricsCache::default();
        assert!(cache.get("x", false).is_ok());
        cache.fail("x", false, MathError::Render("muy grande".into()));
        assert_eq!(
            cache.get("x", false),
            Err(MathError::Render("muy grande".into()))
        );
        assert!(cache.get("x", true).is_ok(), "la de bloque es otra");
    }

    #[test]
    fn la_cache_tiene_tope_y_conserva_lo_que_se_sigue_usando() {
        let cache = MetricsCache::default();
        let _ = cache.get("visible", true);
        for index in 0..GENERATION * 2 {
            let _ = cache.get(&format!("x_{{{index}}}"), true);
            let _ = cache.get("visible", true);
        }
        let generations = cache.generations.lock().unwrap();
        assert!(generations.current[1].len() <= GENERATION);
        assert!(generations.previous[1].len() <= GENERATION);
        assert!(generations.current[1].contains_key("visible"));
    }
}
