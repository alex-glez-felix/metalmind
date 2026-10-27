use gpui_kit::{IntoElement, Render, TestAppContext, Window, anchored, point, px, size};
use metalmind_config::Config;

use super::*;

struct MenuProbe {
    max_width: Pixels,
}

impl Render for MenuProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        let menu = CompletionMenu {
            rows: vec![MenuRow {
                icon: div(),
                name: "Rust".to_owned(),
                detail: Some("alias rustacean-de-toda-la-vida-con-un-alias-muy-largo".to_owned()),
                count: Some("12".to_owned()),
            }],
            visible: 0..1,
            selected: 0,
            accept_keys: Some("tab".to_owned()),
            max_width: self.max_width,
        };
        anchored().child(menu.render(&theme, cx, |_, row, _| row))
    }
}

/// Las cajas de las partes de la fila de `Rust`, que coincide por un alias
/// largo, pintada en no más de `max_width`.
fn laid_out(max_width: Pixels) -> impl Fn(&str) -> Bounds<Pixels> {
    let mut cx = TestAppContext::single();
    let (_, cx) = cx.add_window_view(|_, _| MenuProbe { max_width });
    cx.run_until_parked();
    let boxes: Vec<(String, Bounds<Pixels>)> = ["", "-name", "-detail", "-count", "-keys"]
        .into_iter()
        .map(|part| {
            let selector = format!("completion-0{part}");
            let bounds = cx.debug_bounds(selector.clone().leak()).unwrap();
            (part.to_owned(), bounds)
        })
        .collect();
    move |part| boxes.iter().find(|(name, _)| name == part).unwrap().1
}

/// Un detalle largo (un alias) no saca la lista de la ventana: se recorta
/// el detalle, y el nombre, el conteo y la tecla se leen enteros.
#[test]
fn un_alias_largo_se_recorta_y_no_pasa_del_ancho() {
    let wide = laid_out(px(10_000.));
    let max_width = px(260.);
    assert!(wide("").size.width > max_width);
    let narrow = laid_out(max_width);
    assert!(narrow("").right() <= max_width, "{:?}", narrow(""));
    assert!(narrow("-detail").size.width < wide("-detail").size.width);
    assert_eq!(narrow("-name").size, wide("-name").size);
    assert_eq!(narrow("-count").size, wide("-count").size);
    assert_eq!(narrow("-keys").size, wide("-keys").size);
    assert!(narrow("-keys").right() <= narrow("").right());
}

/// Filas de 10 px con 2 px de marco arriba y abajo: `n` filas miden
/// `10 * n + 4`. La línea mide 10 px desde `top` y el área va de 0 a
/// `height`.
#[rstest::rstest]
#[case::abajo_si_caben_todas(3, 0., 100., 10., 3)]
#[case::abajo_aunque_quepan_justo(3, 56., 100., 66., 3)]
#[case::encima_si_abajo_no_caben(3, 80., 100., 46., 3)]
#[case::sin_lugar_para_todas_del_lado_con_mas(8, 60., 100., 6., 5)]
#[case::sin_lugar_para_todas_abajo_si_hay_mas(8, 30., 100., 40., 5)]
#[case::con_el_mismo_lugar_abajo(8, 45., 100., 55., 4)]
#[case::al_menos_una_fila(3, 0., 12., 10., 1)]
fn la_lista_va_donde_cabe(
    #[case] wanted: usize,
    #[case] top: f32,
    #[case] height: f32,
    #[case] y: f32,
    #[case] rows: usize,
) {
    let metrics = MenuMetrics {
        row_height: px(10.),
        frame: px(2.),
    };
    let line = Bounds::new(point(px(0.), px(top)), size(px(50.), px(10.)));
    let area = Bounds::new(point(px(0.), px(0.)), size(px(200.), px(height)));
    assert_eq!(
        metrics.place(wanted, line, area),
        Placement { y: px(y), rows }
    );
}
