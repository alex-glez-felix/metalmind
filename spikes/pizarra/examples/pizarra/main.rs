//! El visor de la pizarra: abre un `.canvas` (o un `.excalidraw`, pasándolo
//! antes por el esbozo del importador) o un tablero generado, y lo pinta con
//! GPUI sin webview. Con `--banco` recorre un guion fijo y escribe los
//! tiempos de frame.
//!
//! ```sh
//! cargo run --release --example pizarra -- ejemplos/arquitectura.canvas
//! cargo run --release --example pizarra -- --tablero 2000 --banco
//! ```

mod banco;
mod pintar;
mod tema;
mod vista;

use std::process::ExitCode;

use gpui_kit::{App, AppContext, Bounds, WindowBounds, WindowOptions, point, px, size};
use metalmind_spike_pizarra::escena::Style;
use metalmind_spike_pizarra::excalidraw::Scene;
use metalmind_spike_pizarra::formato::Board;
use metalmind_spike_pizarra::{generar, mapeo};

use crate::pintar::Camara;
use crate::tema::{Tema, Variante};
use crate::vista::{Ajustes, Pizarra};

struct Opciones {
    board: Board,
    titulo: String,
    estilo: Style,
    tema: Variante,
    tamano: (f32, f32),
    banco: bool,
    camara: Option<Camara>,
    fuente: String,
    detalle: bool,
}

const USO: &str = "uso: pizarra [ARCHIVO.canvas | ARCHIVO.excalidraw | --tablero N] [--mano] [--claro] [--tam 1920x1080] [--camara x,y,zoom] [--fuente FAMILIA] [--sin-detalle] [--banco]";

fn leer(ruta: &str) -> Result<(Board, String), String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("{ruta}: {e}"))?;
    if ruta.ends_with(".excalidraw") {
        let escena = Scene::parse(&texto).map_err(|e| format!("{ruta}: {e}"))?;
        let (board, saltados) = mapeo::from_excalidraw(&escena);
        for s in saltados {
            eprintln!("{ruta}: no se importó {}: {}", s.id, s.reason);
        }
        return Ok((board, ruta.to_owned()));
    }
    Ok((
        Board::parse(&texto).map_err(|e| format!("{ruta}: {e}"))?,
        ruta.to_owned(),
    ))
}

fn opciones() -> Result<Opciones, String> {
    let mut args = std::env::args().skip(1);
    let mut fuente = None;
    let mut opciones = Opciones {
        board: generar::example(),
        titulo: "el ejemplo".to_owned(),
        estilo: Style::Clean,
        tema: Variante::Oscuro,
        tamano: (1280.0, 720.0),
        banco: false,
        camara: None,
        fuente: String::new(),
        detalle: true,
    };
    while let Some(arg) = args.next() {
        let mut valor = || {
            args.next()
                .ok_or_else(|| format!("falta el valor de {arg}\n{USO}"))
        };
        match arg.as_str() {
            "--tablero" => {
                let n: usize = valor()?.parse().map_err(|_| USO.to_owned())?;
                opciones.board = generar::board(n, 1);
                opciones.titulo = format!("tablero de {n} elementos");
            }
            "--mano" => opciones.estilo = Style::Rough,
            "--claro" => opciones.tema = Variante::Claro,
            "--banco" => opciones.banco = true,
            "--sin-detalle" => opciones.detalle = false,
            "--fuente" => fuente = Some(valor()?),
            "--tam" => {
                let texto = valor()?;
                let (w, h) = texto.split_once('x').ok_or_else(|| USO.to_owned())?;
                opciones.tamano = (
                    w.parse().map_err(|_| USO.to_owned())?,
                    h.parse().map_err(|_| USO.to_owned())?,
                );
            }
            "--camara" => {
                let texto = valor()?;
                let partes: Vec<f32> = texto.split(',').filter_map(|p| p.parse().ok()).collect();
                let [x, y, zoom] = partes[..] else {
                    return Err(USO.to_owned());
                };
                opciones.camara = Some(Camara { x, y, zoom });
            }
            ruta if !ruta.starts_with("--") => (opciones.board, opciones.titulo) = leer(ruta)?,
            _ => return Err(USO.to_owned()),
        }
    }
    opciones.fuente = fuente.unwrap_or_else(|| "Liberation Sans".to_owned());
    Ok(opciones)
}

fn main() -> ExitCode {
    // GPUI avisa por `log` (la GPU elegida, una escena que no cabe).
    env_logger::init();
    let opciones = match opciones() {
        Ok(o) => o,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    gpui_kit::application().run(move |cx: &mut App| {
        let (ancho, alto) = opciones.tamano;
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(ancho), px(alto)));
        let ventana = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // En el benchmark no se baja a 30 fps sin foco: se mide el render.
            inactive_frame_interval: None,
            ..WindowOptions::default()
        };
        let tema = Tema::de(opciones.tema, &opciones.fuente);
        let Opciones {
            board,
            titulo,
            estilo,
            banco,
            camara,
            detalle,
            ..
        } = opciones;
        let ajustes = Ajustes {
            estilo,
            tema,
            titulo,
            banco,
            camara,
            detalle,
        };
        let abierta = cx.open_window(ventana, move |window, cx| {
            let vista = cx.new(|cx| Pizarra::new(board, ajustes, cx));
            let foco = vista.read(cx).foco().clone();
            window.focus(&foco, cx);
            vista
        });
        if let Err(error) = abierta {
            eprintln!("no se pudo abrir la ventana: {error}");
            cx.quit();
        }
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
    ExitCode::SUCCESS
}
