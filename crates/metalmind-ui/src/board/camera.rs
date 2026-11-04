//! Qué parte de la pizarra se ve. Las coordenadas de pantalla van relativas a
//! la esquina del área donde se pinta.

use metalmind_board::{Point, Rect};

const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 8.0;
/// "Ver todo" no agranda una pizarra chica: se queda al 100 %.
const MAX_FIT_ZOOM: f32 = 1.0;
const HALF: f32 = 0.5;

/// El tamaño en pantalla del área donde se pinta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaSize {
    pub width: f32,
    pub height: f32,
}

impl AreaSize {
    fn center(self) -> Point {
        Point::new(self.width * HALF, self.height * HALF)
    }
}

/// `(x, y)` es el punto de la pizarra en la esquina y `zoom`, cuántos píxeles
/// mide una unidad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Camera {
    pub const ACTUAL_SIZE: f32 = 1.0;

    /// Deja `content` entero y centrado, con `margin` alrededor.
    #[must_use]
    pub fn fit(content: Rect, area: AreaSize, margin: f32) -> Self {
        let room = |side: f32, measure: f32| (side - margin - margin).max(1.0) / measure.max(1.0);
        let zoom = room(area.width, content.w)
            .min(room(area.height, content.h))
            .clamp(MIN_ZOOM, MAX_FIT_ZOOM);
        let center = content.center();
        Self {
            x: center.x - area.width * HALF / zoom,
            y: center.y - area.height * HALF / zoom,
            zoom,
        }
    }

    #[must_use]
    pub fn to_screen(self, p: Point) -> Point {
        Point::new((p.x - self.x) * self.zoom, (p.y - self.y) * self.zoom)
    }

    #[must_use]
    pub fn to_board(self, screen: Point) -> Point {
        Point::new(self.x + screen.x / self.zoom, self.y + screen.y / self.zoom)
    }

    #[must_use]
    pub fn visible(self, area: AreaSize) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: area.width / self.zoom,
            h: area.height / self.zoom,
        }
    }

    /// Mueve la vista `(dx, dy)` píxeles de pantalla.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.x += dx / self.zoom;
        self.y += dy / self.zoom;
    }

    /// Cambia el zoom dejando quieto el punto de la pizarra bajo `anchor`.
    pub fn set_zoom_at(&mut self, zoom: f32, anchor: Point) {
        let fixed = self.to_board(anchor);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.x = fixed.x - anchor.x / self.zoom;
        self.y = fixed.y - anchor.y / self.zoom;
    }

    pub fn zoom_center(&mut self, zoom: f32, area: AreaSize) {
        self.set_zoom_at(zoom, area.center());
    }

    /// Mueve lo mínimo para que `rect` quede dentro, con `margin` alrededor; si
    /// no cabe, lo centra.
    pub fn reveal(&mut self, rect: Rect, area: AreaSize, margin: f32) {
        let shown = self.visible(area);
        let margin = margin / self.zoom;
        self.x = reveal_axis(self.x, shown.w, rect.x, rect.w, margin);
        self.y = reveal_axis(self.y, shown.h, rect.y, rect.h, margin);
    }

    #[must_use]
    pub fn percent(self) -> u32 {
        const PERCENT: f32 = 100.0;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "el zoom está entre MIN_ZOOM y MAX_ZOOM"
        )]
        let percent = (self.zoom * PERCENT).round() as u32;
        percent
    }
}

fn reveal_axis(start: f32, shown: f32, at: f32, measure: f32, margin: f32) -> f32 {
    let wanted = measure + margin + margin;
    if wanted > shown {
        at + measure * HALF - shown * HALF
    } else if at - margin < start {
        at - margin
    } else if at + measure + margin > start + shown {
        at + measure + margin - shown
    } else {
        start
    }
}

#[cfg(test)]
#[expect(clippy::float_cmp, reason = "lo que sale de un clamp es exacto")]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    const AREA: AreaSize = AreaSize {
        width: 800.0,
        height: 600.0,
    };

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn pantalla_y_pizarra_son_inversas() {
        let camera = Camera {
            x: -30.0,
            y: 12.0,
            zoom: 2.5,
        };
        let point = Point::new(100.0, -40.0);
        let back = camera.to_board(camera.to_screen(point));
        assert!((back.x - point.x).abs() < 1e-3 && (back.y - point.y).abs() < 1e-3);
    }

    #[test]
    fn ver_todo_deja_el_contenido_dentro_y_centrado() {
        let content = rect(1000.0, 500.0, 4000.0, 1000.0);
        let camera = Camera::fit(content, AREA, 40.0);
        let shown = camera.visible(AREA);
        assert!(shown.x <= content.x && shown.x + shown.w >= content.x + content.w);
        assert!(shown.y <= content.y && shown.y + shown.h >= content.y + content.h);
        let (a, b) = (shown.center(), content.center());
        assert!((a.x - b.x).abs() < 1e-2 && (a.y - b.y).abs() < 1e-2);
    }

    #[test]
    fn ver_todo_no_agranda_una_pizarra_chica() {
        let camera = Camera::fit(rect(0.0, 0.0, 10.0, 10.0), AREA, 40.0);
        assert_eq!(camera.zoom, Camera::ACTUAL_SIZE);
    }

    #[test]
    fn ver_todo_con_un_area_sin_lugar_no_da_un_zoom_invalido() {
        let area = AreaSize {
            width: 10.0,
            height: 0.0,
        };
        let camera = Camera::fit(rect(0.0, 0.0, 500.0, 500.0), area, 40.0);
        assert_eq!(camera.zoom, MIN_ZOOM);
        assert!(camera.x.is_finite() && camera.y.is_finite());
    }

    #[rstest]
    #[case(2.0, 2.0)]
    #[case(100.0, MAX_ZOOM)]
    #[case(0.0, MIN_ZOOM)]
    fn el_zoom_deja_quieto_el_punto_bajo_el_ancla(#[case] asked: f32, #[case] zoom: f32) {
        let mut camera = Camera {
            x: 50.0,
            y: 20.0,
            zoom: 1.0,
        };
        let anchor = Point::new(300.0, 100.0);
        let fixed = camera.to_board(anchor);
        camera.set_zoom_at(asked, anchor);
        assert_eq!(camera.zoom, zoom);
        let back = camera.to_board(anchor);
        assert!((back.x - fixed.x).abs() < 1e-3 && (back.y - fixed.y).abs() < 1e-3);
    }

    #[test]
    fn mover_la_vista_son_pixeles_de_pantalla() {
        let mut camera = Camera {
            x: 0.0,
            y: 0.0,
            zoom: 2.0,
        };
        camera.pan(80.0, -40.0);
        assert_eq!((camera.x, camera.y), (40.0, -20.0));
    }

    #[rstest]
    #[case::ya_se_ve(rect(100.0, 100.0, 50.0, 50.0), (0.0, 0.0))]
    #[case::a_la_derecha(rect(1000.0, 100.0, 50.0, 50.0), (260.0, 0.0))]
    #[case::arriba(rect(100.0, -200.0, 50.0, 50.0), (0.0, -210.0))]
    #[case::no_cabe(rect(0.0, 0.0, 2000.0, 50.0), (600.0, -10.0))]
    fn revelar_mueve_lo_minimo(#[case] node: Rect, #[case] expected: (f32, f32)) {
        let mut camera = Camera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };
        camera.reveal(node, AREA, 10.0);
        assert_eq!((camera.x, camera.y), expected);
    }

    #[rstest]
    #[case(1.0, 100)]
    #[case(0.111, 11)]
    #[case(1.255, 126)]
    fn el_porcentaje_se_redondea(#[case] zoom: f32, #[case] percent: u32) {
        let camera = Camera {
            x: 0.0,
            y: 0.0,
            zoom,
        };
        assert_eq!(camera.percent(), percent);
    }
}
