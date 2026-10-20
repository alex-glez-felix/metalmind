//! Cuánto puede subir el conteo de instrucciones de un benchmark
//! `instrucciones` contra su base antes de que el job de `perf.yml` falle
//! (docs/performance.md, capa 2).

/// Para todos: repetir la corrida mueve el conteo menos de 1 % (0.024 % en el
/// núcleo; 1.6 % como mucho en el índice de links, en 12 corridas), así que el
/// 5 % es margen para regresiones de verdad.
pub const MAX_INSTRUCTION_REGRESSION: f64 = 5.0;
