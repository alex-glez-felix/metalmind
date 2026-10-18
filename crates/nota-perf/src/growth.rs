//! Pruebas de complejidad: correr con N y con 4N y comparar. Los umbrales
//! quedan a medio camino, en escala logarítmica, entre lo esperado y lo
//! siguiente peor, así que el ruido tiene que duplicar el tiempo para fallar.

use std::hint::black_box;
use std::time::{Duration, Instant};

/// Entre lineal (×4) y cuadrático (×16). Un `N log N` da ×4.5–5.
pub const MAX_LINEAR_GROWTH: f64 = 8.0;

/// Para lo que no debería depender del tamaño del texto, como una tecla.
/// Lineal da ×4; un `log N` da ×1, pero hasta ×2 con la caché compartida
/// con el resto de la suite, porque el árbol más grande falla más en caché.
pub const MAX_FLAT_GROWTH: f64 = 3.0;

/// Corridas de cada tamaño. Se alternan y se toma la más rápida: el ruido de
/// otros tests en paralelo solo suma tiempo, nunca lo resta.
const ROUNDS: usize = 11;

/// Por debajo de esto el reloj y el ruido pesan más que el trabajo medido.
const MIN_SMALL: Duration = Duration::from_millis(1);

/// Cuánto más tarda `run` con la entrada de tamaño `4 * n` que con la de `n`.
/// `setup` arma una entrada nueva para cada corrida, fuera de la medición.
///
/// # Panics
/// Si con `n` la corrida es tan corta que la proporción no significa nada.
pub fn growth<T, R>(n: usize, setup: impl Fn(usize) -> T, mut run: impl FnMut(T) -> R) -> f64 {
    let mut small = Duration::MAX;
    let mut large = Duration::MAX;
    // La primera vuelta calienta cachés y asignador; también cuenta, es un mínimo.
    for _ in 0..ROUNDS {
        small = small.min(time(&mut run, setup(n)));
        large = large.min(time(&mut run, setup(4 * n)));
    }
    assert!(
        small >= MIN_SMALL,
        "con n = {n} tarda {small:?}: sube n para que la medición signifique algo"
    );
    large.as_secs_f64() / small.as_secs_f64()
}

/// Falla si `run` crece bastante más que linealmente entre `n` y `4 * n`.
///
/// # Panics
/// Si el crecimiento pasa de [`MAX_LINEAR_GROWTH`].
pub fn assert_linear<T, R>(n: usize, setup: impl Fn(usize) -> T, run: impl FnMut(T) -> R) {
    let ratio = growth(n, setup, run);
    assert!(
        ratio <= MAX_LINEAR_GROWTH,
        "de n = {n} a 4n tardó ×{ratio:.1} (lineal: ×4, cuadrático: ×16, \
         máximo: ×{MAX_LINEAR_GROWTH})"
    );
}

/// Falla si `run` crece con `n` en vez de quedarse casi igual.
///
/// # Panics
/// Si el crecimiento pasa de [`MAX_FLAT_GROWTH`].
pub fn assert_flat<T, R>(n: usize, setup: impl Fn(usize) -> T, run: impl FnMut(T) -> R) {
    let ratio = growth(n, setup, run);
    assert!(
        ratio <= MAX_FLAT_GROWTH,
        "de n = {n} a 4n tardó ×{ratio:.1} (constante: ×1, lineal: ×4, \
         máximo: ×{MAX_FLAT_GROWTH})"
    );
}

fn time<T, R>(run: &mut impl FnMut(T) -> R, input: T) -> Duration {
    let start = Instant::now();
    let output = black_box(run(black_box(input)));
    let elapsed = start.elapsed();
    drop(output);
    elapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(n: usize) -> Vec<u64> {
        (0..n as u64).rev().collect()
    }

    fn sum(values: &[u64]) -> u64 {
        values.iter().sum()
    }

    fn pairs(values: &[u64]) -> usize {
        values
            .iter()
            .map(|a| values.iter().filter(|&b| a < b).count())
            .sum()
    }

    /// El mismo trabajo sea cual sea el tamaño de `values`.
    fn fixed_work(values: &[u64]) -> u64 {
        (0..200_000_u64)
            .zip(values.iter().cycle().take(200_000))
            .map(|(i, value)| value.wrapping_mul(i))
            .sum()
    }

    #[test]
    fn a_linear_pass_is_linear() {
        assert_linear(400_000, numbers, |values| sum(&values));
    }

    #[test]
    #[should_panic(expected = "cuadrático")]
    fn a_quadratic_pass_is_not_linear() {
        assert_linear(700, numbers, |values| pairs(&values));
    }

    #[test]
    fn fixed_work_is_flat() {
        assert_flat(1_000, numbers, |values| fixed_work(&values));
    }

    #[test]
    #[should_panic(expected = "constante")]
    fn a_linear_pass_is_not_flat() {
        assert_flat(400_000, numbers, |values| sum(&values));
    }

    #[test]
    #[should_panic(expected = "sube n")]
    fn a_too_small_input_is_rejected() {
        assert_linear(4, numbers, |values| sum(&values));
    }
}
