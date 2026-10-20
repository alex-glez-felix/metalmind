/// Los registros `"+` y `"*`. Leer y escribir el portapapeles es I/O, así
/// que lo hace quien aloja el motor y se lo pasa en cada llamada.
pub trait Clipboard {
    fn read(&mut self, register: SystemRegister) -> Option<String>;
    fn write(&mut self, register: SystemRegister, text: String);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemRegister {
    /// `"+`: el portapapeles.
    Clipboard,
    /// `"*`: la selección primaria de X11 y Wayland.
    Primary,
}

/// Sin portapapeles del sistema, `"+` y `"*` se guardan como registros
/// normales, como en nvim sin proveedor.
#[derive(Debug, Default)]
pub(crate) struct Detached {
    clipboard: Option<String>,
    primary: Option<String>,
}

impl Detached {
    pub(crate) fn get(&self, register: SystemRegister) -> Option<String> {
        match register {
            SystemRegister::Clipboard => self.clipboard.clone(),
            SystemRegister::Primary => self.primary.clone(),
        }
    }

    pub(crate) fn set(&mut self, register: SystemRegister, text: String) {
        match register {
            SystemRegister::Clipboard => self.clipboard = Some(text),
            SystemRegister::Primary => self.primary = Some(text),
        }
    }
}
