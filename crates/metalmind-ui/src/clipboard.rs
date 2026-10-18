//! El portapapeles de GPUI como los registros `"+` y `"*` de `metalmind-vim`.

use gpui_kit::{App, ClipboardItem};
use metalmind_vim::{Clipboard, SystemRegister};

pub struct SystemClipboard<'a>(pub &'a App);

impl Clipboard for SystemClipboard<'_> {
    fn read(&mut self, register: SystemRegister) -> Option<String> {
        let item = match register {
            SystemRegister::Clipboard => self.0.read_from_clipboard(),
            SystemRegister::Primary => self.0.read_from_primary(),
        };
        item.and_then(|item| item.text())
    }

    fn write(&mut self, register: SystemRegister, text: String) {
        let item = ClipboardItem::new_string(text);
        match register {
            SystemRegister::Clipboard => self.0.write_to_clipboard(item),
            SystemRegister::Primary => self.0.write_to_primary(item),
        }
    }
}
