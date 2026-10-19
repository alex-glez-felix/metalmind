//! Lo que el reporte afirma de cada motor, comprobado.

use metalmind_spike_math::formulas::FORMULAS;
use metalmind_spike_math::motor_typst::TypstEngine;
use metalmind_spike_math::{Image, Kind, Look, MathError, Mode};

fn failures(kind: Kind) -> Result<Vec<&'static str>, MathError> {
    let mut engine = kind.start()?;
    Ok(FORMULAS
        .iter()
        .filter(|f| engine.render(f.latex, f.mode, &Look::default()).is_err())
        .map(|f| f.name)
        .collect())
}

#[test]
fn ratex_compone_las_22() {
    assert_eq!(failures(Kind::Ratex).unwrap(), Vec::<&str>::new());
}

#[test]
fn typst_solo_falla_cd() {
    assert_eq!(
        failures(Kind::Typst).unwrap(),
        ["diagrama conmutativo (amscd)"]
    );
}

#[test]
fn mathml_falla_por_entidades_de_html_y_cd() {
    assert_eq!(
        failures(Kind::Mathml).unwrap(),
        [
            "por casos",
            "épsilon-delta",
            "suma con underbrace",
            "diagrama conmutativo (amscd)"
        ]
    );
    assert_eq!(
        failures(Kind::MathCore).unwrap(),
        [
            "límite",
            "determinante 3×3",
            "densidad normal",
            "diagrama conmutativo (amscd)"
        ]
    );
}

/// Con la spec generada del paquete embebido, `\partial` y `\langle` salen
/// con los nombres de Typst 0.15. Sin el parche de `preparar.sh`, mitex
/// escribe `diff` y `angle.l`.
#[test]
fn mitex_usa_la_spec_que_recibe() {
    let engine = TypstEngine::new().unwrap();
    let code = engine.convert(r"\partial \langle u \rangle").unwrap();
    assert!(code.contains("partial"), "{code}");
    assert!(code.contains("chevron.l"), "{code}");
    assert!(!code.contains("diff"), "{code}");
}

#[test]
fn la_linea_base_cae_dentro_de_la_imagen() {
    for kind in Kind::ALL {
        let mut engine = kind.start().unwrap();
        let image = engine
            .render(r"\frac{a}{b} + x_1", Mode::Inline, &Look::default())
            .unwrap()
            .image;
        #[expect(clippy::cast_precision_loss, reason = "alto chico")]
        let height = image.height as f32;
        assert!(
            image.baseline > 0.0 && image.baseline < height,
            "{}: {}",
            kind.name(),
            image.baseline
        );
    }
}

#[test]
fn la_escala_2_duplica_los_pixeles() {
    let mut engine = Kind::Ratex.start().unwrap();
    let look = Look::default();
    let one = engine.render("x^2", Mode::Inline, &look).unwrap().image;
    let two = engine
        .render("x^2", Mode::Inline, &Look { scale: 2.0, ..look })
        .unwrap()
        .image;
    assert!(
        two.width.abs_diff(one.width * 2) <= 2,
        "{} vs {}",
        one.width,
        two.width
    );
}

#[test]
fn quita_el_premultiplicado_y_pasa_a_bgra() {
    let image = Image::from_premultiplied(2, 1, 0.0, vec![50, 100, 0, 128, 10, 20, 30, 255]);
    assert_eq!(image.rgba, [99, 199, 0, 128, 10, 20, 30, 255]);
    assert_eq!(image.to_bgra(), [0, 199, 99, 128, 30, 20, 10, 255]);
}

#[test]
fn pinta_encima_con_alfa() {
    let mut canvas = Image::filled(2, 1, [255, 255, 255, 255]);
    let top = Image::filled(1, 1, [0, 0, 0, 255]);
    canvas.draw(&top, 1, 0);
    assert_eq!(canvas.rgba, [255, 255, 255, 255, 0, 0, 0, 255]);
}

#[test]
fn pinta_sobre_un_fondo_transparente_sin_perder_color() {
    let mut canvas = Image::filled(1, 1, [0, 0, 0, 0]);
    canvas.draw(&Image::filled(1, 1, [255, 0, 0, 128]), 0, 0);
    assert_eq!(canvas.rgba, [255, 0, 0, 128]);

    let mut half = Image::filled(1, 1, [0, 0, 255, 128]);
    half.draw(&Image::filled(1, 1, [255, 0, 0, 128]), 0, 0);
    assert_eq!(half.rgba, [170, 0, 85, 192]);
}
