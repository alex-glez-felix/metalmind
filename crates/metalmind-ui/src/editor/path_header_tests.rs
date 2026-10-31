use gpui_kit::{
    Bounds, IntoElement, Pixels, Render, TestAppContext, VisualTestContext, Window, px,
};
use metalmind_config::Config;

use super::*;

struct LineProbe {
    width: Pixels,
    note: &'static str,
}

impl Render for LineProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        let note = VaultPath::new(self.note).unwrap();
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

const HONDA: &str =
    "Proyectos/Metalmind/una nota con un nombre larguísimo que no cabe en un panel angosto.md";

/// Las cajas de la carpeta `at` de la ruta de `note` y del nombre, en un
/// panel de `width`.
fn laid_out(note: &'static str, at: usize, width: Pixels) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let mut cx = TestAppContext::single();
    let (_, cx): (_, &mut VisualTestContext) = cx.add_window_view(|_, _| LineProbe { width, note });
    cx.run_until_parked();
    let crumb = format!("pane-path-crumb-{at}");
    (
        cx.debug_bounds(crumb.leak()).unwrap(),
        cx.debug_bounds("pane-path-name").unwrap(),
    )
}

#[test]
fn en_un_panel_ancho_se_ven_enteros() {
    let (nearest, name) = laid_out(HONDA, 1, px(10_000.));
    assert!(name.left() >= nearest.right(), "{nearest:?} {name:?}");
}

/// Un nombre larguísimo no se come la carpeta de la nota: se ve entera,
/// para el clic, y el nombre se recorta sin salirse del panel.
#[rstest::rstest]
#[case::una_ruta_honda(HONDA, 1)]
#[case::una_carpeta_de_una_letra_y_un_nombre_de_255(MAX, 0)]
fn un_nombre_largo_deja_ver_la_carpeta_de_la_nota(#[case] note: &'static str, #[case] at: usize) {
    let (wide, _) = laid_out(note, at, px(10_000.));
    let width = px(300.);
    let (nearest, name) = laid_out(note, at, width);
    assert_eq!(nearest.size, wide.size, "{nearest:?}");
    assert!(nearest.left() >= px(0.), "{nearest:?}");
    assert!(name.left() >= nearest.right(), "{nearest:?} {name:?}");
    assert!(name.right() <= width, "{name:?}");
}

/// `a/` y un nombre de 255 bytes, lo más que deja un sistema de archivos.
const MAX: &str = concat!(
    "a/",
    "una-nota-con-el-nombre-mas-largo-que-deja-el-sistema-de-archivos-",
    "una-nota-con-el-nombre-mas-largo-que-deja-el-sistema-de-archivos-",
    "una-nota-con-el-nombre-mas-largo-que-deja-el-sistema-de-archivos-",
    "una-nota-con-el-nombre-mas-largo-que-deja-el-sistema-de-a.md",
);

/// Una carpeta más ancha que media línea se recorta y deja lugar al nombre.
#[test]
fn una_carpeta_larguisima_no_se_come_el_nombre() {
    let note = "una carpeta con un nombre larguísimo que no cabe ni en media línea/nota.md";
    let width = px(300.);
    let (nearest, name) = laid_out(note, 0, width);
    assert!(nearest.size.width <= width / 2., "{nearest:?}");
    assert!(name.size.width > px(0.), "{name:?}");
    assert!(name.right() <= width, "{name:?}");
}
