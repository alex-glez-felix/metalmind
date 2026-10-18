//! Pruebas de complejidad: correr con N y con 16N y comparar. El paso es ×16
//! y no ×4 porque con ×4 un `N^1.5` da ×8, justo entre lineal (×4) y
//! cuadrático (×16), y ningún umbral lo separa de lo lineal con margen.

use std::hint::black_box;
use std::time::{Duration, Instant};

/// Cuánto más grande es la entrada grande.
pub const FACTOR: usize = 16;

/// Lineal da ×16 y un `N log N`, con la caché en contra, hasta ×25; `N^1.5`
/// da ×64 y cuadrático ×256. El umbral queda a la misma distancia de ×25 y de ×64.
pub const MAX_LINEAR_GROWTH: f64 = 40.0;

/// `16^0.5`, para lo que no debería depender del tamaño del texto, como una
/// tecla: constante da ×1 y lineal ×16.
pub const MAX_FLAT_GROWTH: f64 = 4.0;

/// Corridas de cada tamaño. Se alternan y se toma la más rápida: el ruido de
/// otros tests en paralelo solo suma tiempo, nunca lo resta.
const ROUNDS: usize = 11;

/// Por debajo de esto el reloj y el ruido pesan más que el trabajo medido.
const MIN_SMALL: Duration = Duration::from_millis(1);

/// Cuánto más tarda `run` con la entrada de tamaño `FACTOR * n` que con la de `n`.
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
        large = large.min(time(&mut run, setup(FACTOR * n)));
    }
    assert!(
        small >= MIN_SMALL,
        "con n = {n} tarda {small:?}: sube n para que la medición signifique algo"
    );
    large.as_secs_f64() / small.as_secs_f64()
}

/// Falla si `run` crece bastante más que linealmente entre `n` y `FACTOR * n`.
///
/// # Panics
/// Si el crecimiento pasa de [`MAX_LINEAR_GROWTH`].
pub fn assert_linear<T, R>(n: usize, setup: impl Fn(usize) -> T, run: impl FnMut(T) -> R) {
    let ratio = growth(n, setup, run);
    assert!(
        ratio <= MAX_LINEAR_GROWTH,
        "de n = {n} a {FACTOR}n tardó ×{ratio:.1} (lineal: ×{FACTOR}, \
         cuadrático: ×{}, máximo: ×{MAX_LINEAR_GROWTH})",
        FACTOR * FACTOR
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
        "de n = {n} a {FACTOR}n tardó ×{ratio:.1} (constante: ×1, \
         lineal: ×{FACTOR}, máximo: ×{MAX_FLAT_GROWTH})"
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
        vec![1; n]
    }

    fn sum(values: &[u64]) -> u64 {
        values.iter().sum()
    }

    /// Una pasada completa por cada bloque de `√n` elementos: `n^1.5`. Lo
    /// cuadrático falla con más razón.
    fn pass_per_block(values: &[u64]) -> u64 {
        values
            .chunks(values.len().isqrt())
            .map(|block| block[0] + values.iter().sum::<u64>())
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
        assert_linear(1_000_000, numbers, |values| sum(&values));
    }

    #[test]
    #[should_panic(expected = "cuadrático")]
    fn an_n_to_the_one_and_a_half_pass_is_not_linear() {
        assert_linear(8_000, numbers, |values| pass_per_block(&values));
    }

    #[test]
    fn fixed_work_is_flat() {
        assert_flat(1_000, numbers, |values| fixed_work(&values));
    }

    #[test]
    #[should_panic(expected = "constante")]
    fn a_linear_pass_is_not_flat() {
        assert_flat(1_000_000, numbers, |values| sum(&values));
    }

    #[test]
    #[should_panic(expected = "sube n")]
    fn a_too_small_input_is_rejected() {
        assert_linear(4, numbers, |values| sum(&values));
    }
}
