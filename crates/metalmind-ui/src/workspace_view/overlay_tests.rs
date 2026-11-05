//! Lo que flota sobre los paneles, en una ventana de verdad.

use gpui_kit::{Entity, TestAppContext};
use metalmind_vault::VaultPath;
use pretty_assertions::{assert_eq, assert_ne};

use super::board_tests::{Window, open};
use super::finder::Finder;

impl Window<'_> {
    fn open_finder(&mut self) -> Entity<Finder> {
        self.keys("space f");
        self.open_picker()
    }

    fn open_picker(&mut self) -> Entity<Finder> {
        let open = self
            .view
            .read_with(self.cx, |view, _| view.overlay.as_ref().unwrap().view());
        open.downcast::<Finder>().ok().unwrap()
    }

    fn touch(&mut self, note: &str) {
        let session = self.view.read_with(self.cx, |view, _| view.session.clone());
        session.update(self.cx, |session, cx| {
            session.touch(VaultPath::new(note).unwrap(), cx);
        });
        self.cx.run_until_parked();
    }
}

fn first_note(window: &mut Window<'_>, finder: &Entity<Finder>) -> String {
    finder.read_with(window.cx, |finder, _| {
        finder.delegate().items()[0].0.to_string()
    })
}

/// El finder abierto sigue a la sesión: una nota recién tocada sube.
#[gpui_kit::test]
fn el_finder_abierto_sigue_a_la_sesion(cx: &mut TestAppContext) {
    let mut window = open(cx, "a.md");
    let finder = window.open_finder();
    window.touch("carpeta con espacio/la nota.md");
    assert_eq!(
        first_note(&mut window, &finder),
        "carpeta con espacio/la nota.md"
    );
    window.touch("a.md");
    assert_eq!(first_note(&mut window, &finder), "a.md");
}

/// El picker de una tarjeta nueva comparte tipo con el finder, pero no se
/// rehace con él: su lista es la de cuando se abrió.
#[gpui_kit::test]
fn un_picker_del_mismo_tipo_no_sigue_a_la_sesion(cx: &mut TestAppContext) {
    let mut window = open(cx, "idea.canvas");
    window.view.update_in(window.cx, |view, window, cx| {
        view.open_card_picker(window, cx);
    });
    window.cx.run_until_parked();
    let picker = window.open_picker();
    let first = first_note(&mut window, &picker);
    assert_ne!(first, "carpeta con espacio/la nota.md");
    window.touch("carpeta con espacio/la nota.md");
    assert_eq!(first_note(&mut window, &picker), first);
}
