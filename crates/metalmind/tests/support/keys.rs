//! Teclas en la notación de vim: `ihola<Esc>:wq<CR>`, `<C-r>`, `<C-S-p>`,
//! `<lt>`.

/// Una tecla con su keysym de X11; las mayúsculas y los símbolos que piden
/// shift los resuelve el mapa de teclado del servidor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stroke {
    pub keysym: u32,
    pub ctrl: bool,
}

const ESCAPE: u32 = 0xff1b;
const RETURN: u32 = 0xff0d;
const BACKSPACE: u32 = 0xff08;
const TAB: u32 = 0xff09;
const HOME: u32 = 0xff50;
const LEFT: u32 = 0xff51;
const UP: u32 = 0xff52;
const RIGHT: u32 = 0xff53;
const DOWN: u32 = 0xff54;

/// Un `<` que no abre una tecla con nombre es un error: el literal es `<lt>`.
pub fn parse(notation: &str) -> Vec<Stroke> {
    let mut strokes = Vec::new();
    let mut rest = notation;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            let end = rest
                .find('>')
                .unwrap_or_else(|| panic!("falta `>` en {notation:?}; el `<` literal es `<lt>`"));
            strokes.push(named(&rest[1..end], notation));
            rest = &rest[end + 1..];
        } else {
            strokes.push(plain(char_keysym(c)));
            rest = &rest[c.len_utf8()..];
        }
    }
    strokes
}

fn named(name: &str, notation: &str) -> Stroke {
    // Con shift va la mayúscula: el mapa de teclado del servidor pone el shift.
    if let Some(key) = name.strip_prefix("C-S-") {
        let mut chars = key.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Stroke {
                keysym: char_keysym(c.to_ascii_uppercase()),
                ctrl: true,
            };
        }
    }
    if let Some(key) = name.strip_prefix("C-").or_else(|| name.strip_prefix("c-")) {
        let mut chars = key.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Stroke {
                keysym: char_keysym(c.to_ascii_lowercase()),
                ctrl: true,
            };
        }
    }
    let keysym = match name.to_ascii_lowercase().as_str() {
        "esc" => ESCAPE,
        "cr" | "enter" | "return" => RETURN,
        "bs" => BACKSPACE,
        "tab" => TAB,
        "space" => char_keysym(' '),
        "lt" => char_keysym('<'),
        "home" => HOME,
        "left" => LEFT,
        "up" => UP,
        "right" => RIGHT,
        "down" => DOWN,
        _ => panic!("tecla desconocida <{name}> en {notation:?}"),
    };
    plain(keysym)
}

fn plain(keysym: u32) -> Stroke {
    Stroke {
        keysym,
        ctrl: false,
    }
}

/// Latin-1 tiene el keysym igual al código; el resto de Unicode va desplazado.
fn char_keysym(c: char) -> u32 {
    match u32::from(c) {
        code @ (0x20..=0x7e | 0xa0..=0xff) => code,
        code => 0x0100_0000 + code,
    }
}

#[test]
fn notacion_de_teclas() {
    let ctrl = |keysym| Stroke { keysym, ctrl: true };
    let cases: [(&str, Vec<Stroke>); 8] = [
        ("iA", vec![plain(0x69), plain(0x41)]),
        (
            "<Esc>:w<CR>",
            vec![plain(ESCAPE), plain(0x3a), plain(0x77), plain(RETURN)],
        ),
        ("<esc><Enter>", vec![plain(ESCAPE), plain(RETURN)]),
        ("<C-r><C-W>", vec![ctrl(0x72), ctrl(0x77)]),
        ("<C-S-p>", vec![ctrl(0x50)]),
        ("<lt>x>", vec![plain(0x3c), plain(0x78), plain(0x3e)]),
        ("ñ€", vec![plain(0xf1), plain(0x0100_20ac)]),
        (
            "<BS><Tab><Space><Up>",
            vec![plain(BACKSPACE), plain(TAB), plain(0x20), plain(UP)],
        ),
    ];
    for (notation, expected) in cases {
        assert_eq!(parse(notation), expected, "{notation:?}");
    }
}

#[test]
#[should_panic(expected = "tecla desconocida <Raro>")]
fn una_tecla_desconocida_falla() {
    parse("<Raro>");
}

#[test]
#[should_panic(expected = "el `<` literal es `<lt>`")]
fn un_menor_suelto_falla() {
    parse("a<b");
}
