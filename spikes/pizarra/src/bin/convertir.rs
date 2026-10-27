//! Esbozo del convertidor entre `.excalidraw` y la pizarra (`.canvas`). Va
//! fuera de la app (09 §2.5): la pizarra solo lee y escribe su formato.
//!
//! ```sh
//! cargo run --release --bin convertir -- entrada.excalidraw salida.canvas
//! cargo run --release --bin convertir -- entrada.canvas salida.excalidraw
//! cargo run --release --bin convertir -- --ejemplo ejemplos/arquitectura.canvas
//! ```

use std::process::ExitCode;

use metalmind_spike_pizarra::excalidraw::Scene;
use metalmind_spike_pizarra::formato::Board;
use metalmind_spike_pizarra::{generar, mapeo};

fn convertir(entrada: &str, salida: &str) -> Result<String, String> {
    if entrada == "--ejemplo" {
        return Ok(generar::example().write());
    }
    let texto = std::fs::read_to_string(entrada).map_err(|e| format!("{entrada}: {e}"))?;
    match (entrada.rsplit('.').next(), salida.rsplit('.').next()) {
        (Some("excalidraw"), Some("canvas")) => {
            let escena = Scene::parse(&texto).map_err(|e| format!("{entrada}: {e}"))?;
            let (board, saltados) = mapeo::from_excalidraw(&escena);
            for s in &saltados {
                eprintln!("{}: {}", s.id, s.reason);
            }
            let texto = board.write();
            // Lo que se escribe tiene que poder leerse: si no, es un error del importador.
            Board::parse(&texto)
                .map_err(|e| format!("la conversión no es un .canvas válido: {e}"))?;
            Ok(texto)
        }
        (Some("canvas"), Some("excalidraw")) => {
            let board = Board::parse(&texto).map_err(|e| format!("{entrada}: {e}"))?;
            Ok(mapeo::to_excalidraw(&board).write())
        }
        _ => Err("convierte .excalidraw → .canvas o .canvas → .excalidraw".to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [entrada, salida] = args.as_slice() else {
        eprintln!("uso: convertir ENTRADA SALIDA  (o --ejemplo SALIDA.canvas)");
        return ExitCode::FAILURE;
    };
    match convertir(entrada, salida)
        .and_then(|texto| std::fs::write(salida, texto).map_err(|e| format!("{salida}: {e}")))
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
