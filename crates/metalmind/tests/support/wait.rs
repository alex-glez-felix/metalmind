//! Esperas con tope: se consulta una condición hasta que se cumpla o se acabe
//! el tiempo, nunca un `sleep` fijo que adivine cuánto tarda la app.

use std::thread;
use std::time::{Duration, Instant};

/// Lavapipe en una máquina de CI tarda segundos en levantar la primera ventana
/// de un build de debug.
pub const WINDOW_TIMEOUT: Duration = Duration::from_mins(1);
pub const TIMEOUT: Duration = Duration::from_secs(15);

const POLL: Duration = Duration::from_millis(10);

/// Lo que devuelva `probe` en cuanto dé `Ok`; si se acaba `timeout`, falla
/// con lo que se esperaba y el último `Err`, que dice lo que se vio.
pub fn until<T>(what: &str, timeout: Duration, mut probe: impl FnMut() -> Result<T, String>) -> T {
    let deadline = Instant::now() + timeout;
    loop {
        match probe() {
            Ok(value) => return value,
            Err(seen) if Instant::now() >= deadline => {
                panic!("pasaron {timeout:?} esperando {what}; lo último: {seen}")
            }
            Err(_) => thread::sleep(POLL),
        }
    }
}
