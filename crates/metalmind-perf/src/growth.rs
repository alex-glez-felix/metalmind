//! Pruebas de complejidad: correr con N y con 16N y comparar las
//! instrucciones. El paso es ×16 y no ×4 porque con ×4 un `N^1.5` da ×8,
//! justo entre lineal (×4) y cuadrático (×16), y ningún umbral lo separa de lo
//! lineal con margen.
//!
//! Cada tamaño se cuenta en un proceso aparte: el mismo binario de tests,
//! corriendo solo esa prueba bajo callgrind. Contar instrucciones es
//! determinista, así que las pruebas pueden correr en paralelo sin repartirse
//! nada que se mida.

mod callgrind;

use std::cell::Cell;
use std::hint::black_box;
use std::thread;

use self::callgrind::{CountError, child, instructions};

/// Cuánto más grande es la entrada grande.
pub const FACTOR: usize = 16;

/// Lineal da ×16 y un `N log N` hasta ×22 (los tags); `N^1.5` da ×64 y
/// cuadrático ×256. En instrucciones no hay caché ni ruido que absorber.
pub const MAX_LINEAR_GROWTH: f64 = 32.0;

/// `16^0.5`, para lo que no debería depender del tamaño del texto, como una
/// tecla: constante da ×1 y lineal ×16.
pub const MAX_FLAT_GROWTH: f64 = 4.0;

/// Por debajo de esto, lo fijo de cada corrida pesa más que el trabajo medido.
const MIN_SMALL: u64 = 100_000;

thread_local! {
    /// Cuántas mediciones lleva la prueba de este hilo.
    static CALLS: Cell<usize> = const { Cell::new(0) };
}

/// Cuántas más instrucciones cuesta `run` con la entrada de tamaño
/// `FACTOR * n` que con la de `n`. `setup` arma la entrada, fuera de la cuenta.
///
/// # Panics
/// Si no corre dentro de una prueba de libtest, si callgrind falla o si con
/// `n` la corrida es tan corta que la proporción no significa nada.
pub fn growth<T, R>(n: usize, setup: impl Fn(usize) -> T, mut run: impl FnMut(T) -> R) -> f64 {
    let call = CALLS.with(|calls| calls.replace(calls.get() + 1));
    if let Some((target, size)) = child() {
        if call == target {
            // Sin soltar la salida: no cuenta, y soltar una ventana espera a
            // los hilos del vault. Lo que siga en la prueba es del padre.
            std::mem::forget(measured(&mut run, setup(size)));
            std::process::exit(0);
        }
        return 1.0;
    }
    let (small, large) = match count(call, n) {
        Ok(counts) => counts,
        Err(error) => panic!("{error}"),
    };
    assert!(
        small >= MIN_SMALL,
        "con n = {n} cuesta {small} instrucciones: sube n para que la medición signifique algo"
    );
    #[allow(
        clippy::cast_precision_loss,
        reason = "una proporción; 2^52 instrucciones no caben en ninguna prueba"
    )]
    let ratio = large as f64 / small as f64;
    eprintln!("de n = {n} a {FACTOR}n: {small} → {large} instrucciones, ×{ratio:.2}");
    ratio
}

/// Las instrucciones con `n` y con `FACTOR * n`, cada una en su proceso y a la vez.
fn count(call: usize, n: usize) -> Result<(u64, u64), CountError> {
    let current = thread::current();
    let test = current
        .name()
        .filter(|name| *name != "main")
        .ok_or(CountError::NoTestName)?;
    thread::scope(|scope| {
        let large = scope.spawn(|| instructions(test, call, FACTOR * n));
        let small = instructions(test, call, n)?;
        let large = large
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))?;
        Ok((small, large))
    })
}

/// Falla si `run` crece bastante más que linealmente entre `n` y `FACTOR * n`.
///
/// # Panics
/// Si el crecimiento pasa de [`MAX_LINEAR_GROWTH`].
pub fn assert_linear<T, R>(n: usize, setup: impl Fn(usize) -> T, run: impl FnMut(T) -> R) {
    let ratio = growth(n, setup, run);
    assert!(
        ratio <= MAX_LINEAR_GROWTH,
        "de n = {n} a {FACTOR}n costó ×{ratio:.1} en instrucciones (lineal: ×{FACTOR}, \
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
        "de n = {n} a {FACTOR}n costó ×{ratio:.1} en instrucciones (constante: ×1, \
         lineal: ×{FACTOR}, máximo: ×{MAX_FLAT_GROWTH})"
    );
}

#[inline(never)]
fn measured<T, R>(run: &mut impl FnMut(T) -> R, input: T) -> R {
    black_box(run(black_box(input)))
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
    #[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
    fn a_linear_pass_is_linear() {
        assert_linear(1_000_000, numbers, |values| sum(&values));
    }

    #[test]
    #[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
    #[should_panic(expected = "cuadrático")]
    fn an_n_to_the_one_and_a_half_pass_is_not_linear() {
        assert_linear(8_000, numbers, |values| pass_per_block(&values));
    }

    #[test]
    #[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
    fn fixed_work_is_flat() {
        assert_flat(1_000, numbers, |values| fixed_work(&values));
    }

    #[test]
    #[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
    #[should_panic(expected = "constante")]
    fn a_linear_pass_is_not_flat() {
        assert_flat(1_000_000, numbers, |values| sum(&values));
    }

    #[test]
    #[cfg_attr(not(valgrind_available), ignore = "valgrind no está en el PATH")]
    #[should_panic(expected = "sube n")]
    fn a_too_small_input_is_rejected() {
        assert_linear(4, numbers, |values| sum(&values));
    }
}
