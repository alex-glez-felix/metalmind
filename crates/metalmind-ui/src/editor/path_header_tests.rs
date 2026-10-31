use gpui_kit::{
    Bounds, IntoElement, Pixels, Render, TestAppContext, VisualTestContext, Window, px,
};
use metalmind_config::Config;

use super::*;

struct LineProbe {
    width: Pixels,
}

impl Render for LineProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        let note = VaultPath::new(
            "Proyectos/Metalmind/una nota con un nombre larguísimo que no cabe en un panel angosto.md",
        )
        .unwrap();
        let line = PathLine {
            pane: crate::workspace::PaneTree::new().focus(),
            path: PanePath::new(Some(&note), ""),
            full: note.to_string().into(),
            focused: true,
            key: None,
        };
        div().w(self.width).child(line.render(&theme))
    }
}

/// Las cajas de las migas y del nombre en un panel de `width`.
fn laid_out(width: Pixels) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let mut cx = TestAppContext::single();
    let (_, cx): (_, &mut VisualTestContext) = cx.add_window_view(|_, _| LineProbe { width });
    cx.run_until_parked();
    (
        cx.debug_bounds("pane-path-crumbs").unwrap(),
        cx.debug_bounds("pane-path-name").unwrap(),
    )
}

#[test]
fn en_un_panel_ancho_se_ven_enteros() {
    let (crumbs, name) = laid_out(px(10_000.));
    assert!(name.left() >= crumbs.right(), "{crumbs:?} {name:?}");
}

/// Un nombre larguísimo no se come las migas: las dos partes se recortan
/// y la carpeta más cercana sigue ahí para el clic.
#[test]
fn un_nombre_largo_deja_ver_las_migas() {
    let (wide_crumbs, _) = laid_out(px(10_000.));
    let width = px(300.);
    let (crumbs, name) = laid_out(width);
    assert!(crumbs.size.width > px(0.), "{crumbs:?}");
    assert!(crumbs.size.width < wide_crumbs.size.width, "{crumbs:?}");
    assert!(name.right() <= width, "{name:?}");
}
