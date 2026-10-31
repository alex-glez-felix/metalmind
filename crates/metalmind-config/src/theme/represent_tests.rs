use super::*;
use crate::Config;

/// Los colores como su `0xrrggbb` y las medidas como píxeles: cambia el
/// tipo de dos unidades y deja igual las demás.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Probe;

impl Units for Probe {}

impl Represent<Color> for Probe {
    type Value = u32;

    fn represent(color: Color) -> u32 {
        color.rgb
    }
}

impl Represent<Length> for Probe {
    type Value = Px;

    fn represent(length: Length) -> Px {
        Px(length.0)
    }
}

macro_rules! unchanged {
    ($($ty:ty),*) => {$(
        impl Represent<$ty> for Probe {
            type Value = $ty;

            fn represent(value: $ty) -> $ty {
                value
            }
        }
    )*};
}

unchanged!(
    Px,
    Factor,
    Fraction,
    Count,
    Weight,
    MarkColumns,
    TextColumns,
    TextStyle
);

#[test]
fn convert_pasa_cada_campo_por_su_unidad() {
    let theme = Config::defaults().unwrap().theme;
    let syntax = theme.syntax.convert::<Probe>();
    assert_eq!(syntax.heading, theme.syntax.heading.rgb);
    assert_eq!(syntax.markup, theme.syntax.markup.rgb);
    let layout = theme.metrics.layout.convert::<Probe>();
    assert_eq!(layout.picker_gap, Px(theme.metrics.layout.picker_gap.0));
    assert_eq!(layout.picker_rows, theme.metrics.layout.picker_rows);
    assert_eq!(layout.icon_size, theme.metrics.layout.icon_size);
}

#[test]
fn en_las_unidades_de_config_nada_cambia() {
    let theme = Config::defaults().unwrap().theme;
    assert_eq!(theme.ui.convert::<Plain>(), theme.ui);
    assert_eq!(theme.metrics.text.convert::<Plain>(), theme.metrics.text);
    assert_eq!(
        theme.metrics.layout.convert::<Plain>(),
        theme.metrics.layout
    );
}
