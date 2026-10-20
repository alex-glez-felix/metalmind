//! Cuánto puede subir el conteo de instrucciones de un benchmark
//! `instrucciones` contra su base antes de que el job de `perf.yml` falle
//! (docs/performance.md, capa 2).

/// Para todos: repetir la corrida mueve el conteo menos de 1 % (0.024 % en el
/// núcleo), así que el 5 % es margen para regresiones de verdad.
pub const MAX_INSTRUCTION_REGRESSION: f64 = 5.0;

/// Para los que varían entre corridas porque recorren un `HashMap` cuyo orden
/// cambia con la semilla de cada proceso: hasta 2.9 % en tres corridas
/// (cambio del watcher en una nota), 1.9 % (`gd`) y 1.5 % (mover una carpeta).
pub const MAX_NOISY_INSTRUCTION_REGRESSION: f64 = 10.0;
