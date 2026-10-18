//! Cuánto tarda cada frame: del arranque al primero, y de cada tecla al
//! frame que la muestra. El primero siempre va al log; con `--frame-log`
//! cada frame se escribe en un archivo para los benchmarks (docs/performance.md).

use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufWriter, Write as _};
use std::path::Path;
use std::time::{Duration, Instant};

use gpui_kit::{App, Global};

/// Una línea por frame presentado, con los tiempos en µs:
/// `frame=3 t=412000 cpu=850 present=4100 input=4300 keys=1`. `t` cuenta
/// desde el arranque; `cpu` va del `render` de la raíz al último `paint` del
/// editor, `present` hasta que volvió `Window::present` (el frame va al
/// renderer, no está aún en pantalla), e `input` (si hubo teclas) desde la
/// primera de las `keys` que el frame muestra hasta ese momento.
pub struct FrameClock {
    launched: Instant,
    frames: u64,
    pending: Pending,
    log: Option<BufWriter<File>>,
}

#[derive(Default)]
struct Pending {
    input: Option<Instant>,
    keys: u32,
    render: Option<Instant>,
    painted: Option<Instant>,
}

impl Global for FrameClock {}

impl FrameClock {
    /// `log` es el archivo de `--frame-log`; se trunca si existe.
    ///
    /// # Errors
    /// Si no se puede crear el archivo.
    pub fn new(launched: Instant, log: Option<&Path>) -> io::Result<Self> {
        let log = log.map(File::create).transpose()?.map(BufWriter::new);
        Ok(Self {
            launched,
            frames: 0,
            pending: Pending::default(),
            log,
        })
    }

    /// Llegó una tecla; si ya había otras sin pintar, el tiempo es el de la primera.
    pub fn input(cx: &mut App) {
        if let Some(clock) = Self::get(cx) {
            clock.pending.keys += 1;
            clock.pending.input.get_or_insert_with(Instant::now);
        }
    }

    /// Empieza el `render` de la raíz: el primero del frame.
    pub fn render_started(cx: &mut App) {
        if let Some(clock) = Self::get(cx) {
            clock.pending.render.get_or_insert_with(Instant::now);
        }
    }

    /// Terminó el `paint` de un editor. Lo diferido corre al salir del
    /// `update` de GPUI que llama a `draw` y `present`; el primer frame lo
    /// dibuja `open_window`, que no lo presenta, así que ahí mide el dibujo.
    pub fn painted(cx: &mut App) {
        let Some(clock) = Self::get(cx) else {
            return;
        };
        if clock.pending.painted.replace(Instant::now()).is_none() {
            cx.defer(|cx| {
                if let Some(clock) = Self::get(cx) {
                    clock.presented(Instant::now());
                }
            });
        }
    }

    fn get(cx: &mut App) -> Option<&mut Self> {
        cx.has_global::<Self>().then(|| cx.global_mut::<Self>())
    }

    fn presented(&mut self, now: Instant) {
        let pending = std::mem::take(&mut self.pending);
        self.frames += 1;
        if self.frames == 1 {
            log::info!(
                "primer frame a {} ms del arranque",
                (now - self.launched).as_millis()
            );
        }
        let Some(log) = &mut self.log else {
            return;
        };
        let line = frame_line(self.frames, now - self.launched, &pending, now);
        if let Err(error) = writeln!(log, "{line}").and_then(|()| log.flush()) {
            log::warn!("no se pudo escribir el registro de frames: {error}");
            self.log = None;
        }
    }
}

fn frame_line(frame: u64, since_launch: Duration, pending: &Pending, now: Instant) -> String {
    let mut line = format!("frame={frame} t={}", since_launch.as_micros());
    if let (Some(render), Some(painted)) = (pending.render, pending.painted) {
        let _ = write!(
            line,
            " cpu={} present={}",
            (painted - render).as_micros(),
            (now - render).as_micros()
        );
    }
    if let Some(input) = pending.input {
        let _ = write!(
            line,
            " input={} keys={}",
            (now - input).as_micros(),
            pending.keys
        );
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_linea_por_frame() {
        let start = Instant::now();
        let at = |micros| start + Duration::from_micros(micros);
        let pending = Pending {
            input: Some(at(100)),
            keys: 2,
            render: Some(at(300)),
            painted: Some(at(1_100)),
        };
        assert_eq!(
            frame_line(4, Duration::from_millis(250), &pending, at(5_100)),
            "frame=4 t=250000 cpu=800 present=4800 input=5000 keys=2"
        );
        let idle = Pending {
            input: None,
            keys: 0,
            ..pending
        };
        assert_eq!(
            frame_line(5, Duration::from_millis(250), &idle, at(5_100)),
            "frame=5 t=250000 cpu=800 present=4800"
        );
    }

    #[test]
    fn el_archivo_tiene_un_frame_por_linea() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frames.log");
        let mut clock = FrameClock::new(Instant::now(), Some(&path)).unwrap();
        clock.presented(Instant::now());
        clock.pending.input = Some(Instant::now());
        clock.pending.keys = 1;
        clock.presented(Instant::now());
        let written = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), 2, "{written}");
        assert!(lines[0].starts_with("frame=1 t="), "{written}");
        assert!(lines[1].starts_with("frame=2 t="), "{written}");
        assert!(lines[1].contains(" input="), "{written}");
        assert!(lines[1].ends_with(" keys=1"), "{written}");
    }
}
