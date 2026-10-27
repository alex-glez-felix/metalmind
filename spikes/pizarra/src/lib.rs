//! Spike de la pizarra (`docs/research/09-pizarra.md`): lo que sería el
//! núcleo, sin GPUI. El formato en disco, por dónde pasan las flechas, los
//! puntos del lápiz, lo que se pinta de cada elemento y, aparte, un `.excalidraw`
//! para comparar y para el esbozo del importador. La ventana está en
//! `examples/pizarra.rs`.

pub mod escena;
pub mod excalidraw;
pub mod formato;
pub mod generar;
pub mod geometria;
pub mod mapeo;
pub mod trazo;
