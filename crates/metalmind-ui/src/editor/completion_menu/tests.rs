use std::sync::Arc;

use gpui_kit::{Bounds, IntoElement, Render, TestAppContext, Window, anchored, px};
use metalmind_config::Config;
use metalmind_search::FuzzyMatcher;
use metalmind_syntax::TagPlace;
use metalmind_vault::tags::{TagCompletion, TagKey};

use super::*;

fn tag(name: &str, count: usize, aliases: &[&str]) -> TagCompletion {
    TagCompletion {
        key: TagKey::new(name),
        name: name.to_owned(),
        aliases: aliases.iter().map(|&alias| alias.to_owned()).collect(),
        count,
        color: None,
        icon: None,
    }
}

struct MenuProbe {
    list: TagCompletions,
    max_width: Pixels,
}

impl Render for MenuProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::with_font(&Config::defaults().unwrap(), "Mono".into());
        let menu = CompletionMenu {
            list: &self.list,
            visible: 0..1,
            accept_keys: Some("tab".to_owned()),
            max_width: self.max_width,
        };
        anchored().child(menu.render(&theme, cx, |_, row, _| row))
    }
}

/// Las cajas de las partes de la fila de `Rust`, que coincide por un alias
/// largo, pintada en no más de `max_width`.
fn laid_out(max_width: Pixels) -> impl Fn(&str) -> Bounds<Pixels> {
    let alias = "rustacean-de-toda-la-vida-con-un-alias-muy-largo";
    let candidates: Arc<[TagCompletion]> = Arc::from([tag("Rust", 12, &[alias])]);
    let list = TagCompletions::open(
        candidates,
        "rustacean",
        TagPlace::Inline,
        false,
        &mut FuzzyMatcher::text(),
    )
    .unwrap();
    let mut cx = TestAppContext::single();
    let (_, cx) = cx.add_window_view(|_, _| MenuProbe { list, max_width });
    cx.run_until_parked();
    let boxes: Vec<(String, Bounds<Pixels>)> = ["", "-name", "-alias", "-count", "-keys"]
        .into_iter()
        .map(|part| {
            let selector = format!("tag-completion-0{part}");
            let bounds = cx.debug_bounds(selector.clone().leak()).unwrap();
            (part.to_owned(), bounds)
        })
        .collect();
    move |part| boxes.iter().find(|(name, _)| name == part).unwrap().1
}

/// Un alias largo no saca la lista de la ventana: se recorta el alias, y el
/// nombre, el conteo y la tecla se leen enteros.
#[test]
fn un_alias_largo_se_recorta_y_no_pasa_del_ancho() {
    let wide = laid_out(px(10_000.));
    let max_width = px(260.);
    assert!(wide("").size.width > max_width);
    let narrow = laid_out(max_width);
    assert!(narrow("").right() <= max_width, "{:?}", narrow(""));
    assert!(narrow("-alias").size.width < wide("-alias").size.width);
    assert_eq!(narrow("-name").size, wide("-name").size);
    assert_eq!(narrow("-count").size, wide("-count").size);
    assert_eq!(narrow("-keys").size, wide("-keys").size);
    assert!(narrow("-keys").right() <= narrow("").right());
}
