//! Las fórmulas de la lista en cada motor, una fila por fórmula y una
//! columna por motor, en un PNG:
//!
//! `galeria <salida.png> [escala]`

use std::path::PathBuf;
use std::process::ExitCode;

use metalmind_spike_math::formulas::FORMULAS;
use metalmind_spike_math::motor_typst::TypstEngine;
use metalmind_spike_math::{Engine, Image, Kind, Look, MathError, Mode};

const GAP: f32 = 12.0;
const BACKGROUND: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const FAILED: [u8; 3] = [0xb4, 0x22, 0x22];
const MUTED: [u8; 3] = [0x6a, 0x73, 0x7d];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("uso: galeria <salida.png> [escala]");
        return ExitCode::FAILURE;
    };
    let scale = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    match gallery(scale).and_then(|image| {
        image
            .save_png(&output)
            .map_err(|e| MathError::Render(e.to_string()))
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Las celdas: una fila de títulos y una fila por fórmula.
fn gallery(scale: f32) -> Result<Image, MathError> {
    let look = Look {
        scale,
        ..Look::default()
    };
    let label_look = Look {
        color: MUTED,
        size: 13.0,
        ..look
    };
    let mut labels = TypstEngine::new()?;
    let mut label = |text: &str, look: &Look| {
        labels
            .render(&format!(r"\text{{{text}}}"), Mode::Inline, look)
            .map(|rendered| rendered.image)
    };
    let mut header = vec![label("fórmula", &label_look)?];
    for kind in Kind::ALL {
        header.push(label(kind.name(), &label_look)?);
    }
    let mut rows = vec![header];
    let mut engines = Kind::ALL.map(Kind::start);
    for (index, formula) in FORMULAS.iter().enumerate() {
        let mut row = vec![label(
            &format!("{}. {}", index + 1, formula.name),
            &label_look,
        )?];
        for engine in &mut engines {
            let cell = match engine {
                Ok(engine) => engine
                    .render(formula.latex, formula.mode, &look)
                    .map(|r| r.image),
                Err(error) => Err(MathError::Layout(error.to_string())),
            };
            row.push(match cell {
                Ok(image) => image,
                Err(_) => label(
                    "falla",
                    &Look {
                        color: FAILED,
                        ..label_look
                    },
                )?,
            });
        }
        rows.push(row);
    }
    Ok(compose(&rows, scale))
}

fn compose(rows: &[Vec<Image>], scale: f32) -> Image {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "un margen chico en píxeles"
    )]
    let gap = (GAP * scale) as u32;
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<u32> = (0..columns)
        .map(|c| {
            rows.iter()
                .filter_map(|row| row.get(c))
                .map(|i| i.width)
                .max()
                .unwrap_or(0)
        })
        .collect();
    let heights: Vec<u32> = rows
        .iter()
        .map(|row| row.iter().map(|i| i.height).max().unwrap_or(0))
        .collect();
    let width = widths.iter().map(|w| w + gap).sum::<u32>() + gap;
    let height = heights.iter().map(|h| h + gap).sum::<u32>() + gap;
    let mut canvas = Image::filled(width, height, BACKGROUND);
    let mut y = gap;
    for (row, row_height) in rows.iter().zip(&heights) {
        let mut x = gap;
        for (cell, cell_width) in row.iter().zip(&widths) {
            canvas.draw(cell, x, y + (row_height - cell.height) / 2);
            x += cell_width + gap;
        }
        y += row_height + gap;
    }
    canvas
}
