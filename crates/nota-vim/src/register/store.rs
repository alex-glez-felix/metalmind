use super::{Clipboard, Register, RegisterName};

/// Qué operador escribe en los registros, para saber a cuáles va.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Write {
    Yank,
    Delete {
        /// Por líneas o de más de una línea.
        spans_lines: bool,
        /// Va a `"1` aunque no pase de una línea.
        always_numbered: bool,
    },
}

/// Todos los registros que guarda el motor. `"+` y `"*` los guarda quien
/// lo aloja.
#[derive(Debug, Default)]
pub(crate) struct Registers {
    /// Copia de lo último que se escribió, como el `y_previous` de vim.
    unnamed: Option<Register>,
    named: [Option<Register>; 26],
    yank: Option<Register>,
    numbered: [Option<Register>; 9],
    small_delete: Option<Register>,
}

impl Registers {
    /// Lo que copió o borró un operador, repartido como `op_yank` y
    /// `op_delete` de vim: un yank sin registro va a `"0`; un borrado de
    /// líneas recorre `"1`–`"9`, y uno dentro de una línea sin registro va a
    /// `"-` (y también a `"1` si se hizo con `` ` ``). Lo último escrito
    /// queda en el registro sin nombre.
    pub fn write(
        &mut self,
        name: Option<RegisterName>,
        register: Register,
        write: Write,
        clipboard: &mut dyn Clipboard,
    ) {
        if name.is_some_and(|name| name == RegisterName::BlackHole || !name.is_writable()) {
            return;
        }
        let mut last = name.map(|name| self.store(name, register.clone(), clipboard));
        match write {
            Write::Yank if name.is_none() => {
                self.yank = Some(register.clone());
                last = Some(register);
            }
            Write::Yank => {}
            Write::Delete {
                spans_lines,
                always_numbered,
            } => {
                if spans_lines || always_numbered {
                    self.numbered.rotate_right(1);
                    self.numbered[0] = Some(register.clone());
                    if !name.is_some_and(RegisterName::appends) {
                        last = Some(register.clone());
                    }
                }
                if name.is_none() && !spans_lines {
                    self.small_delete = Some(register.clone());
                    last = Some(register);
                }
            }
        }
        if last.is_some() {
            self.unnamed = last;
        }
    }

    /// Lo grabado con `q` en `"a`–`"z`. Como en vim, no pasa por el
    /// registro sin nombre, y con `qA` se pega al final de la última línea.
    pub fn record(&mut self, letter: u8, append: bool, keys: String) {
        let slot = &mut self.named[usize::from(letter)];
        match slot {
            Some(old) if append => old.text.push_str(&keys),
            _ => {
                *slot = Some(Register {
                    text: keys,
                    linewise: false,
                });
            }
        }
    }

    /// `None` si está vacío o no se puede leer como texto.
    pub fn read(
        &self,
        name: Option<RegisterName>,
        clipboard: &mut dyn Clipboard,
    ) -> Option<Register> {
        let Some(name) = name else {
            return self.unnamed.clone();
        };
        match name {
            RegisterName::Unnamed => self.unnamed.clone(),
            RegisterName::Named(letter) | RegisterName::Append(letter) => {
                self.named[usize::from(letter)].clone()
            }
            RegisterName::Yank => self.yank.clone(),
            RegisterName::Numbered(n) => self.numbered[usize::from(n - 1)].clone(),
            RegisterName::SmallDelete => self.small_delete.clone(),
            RegisterName::System(system) => {
                clipboard.read(system).and_then(Register::from_clipboard)
            }
            // `".` se pega volviendo a teclearlo y `"/` lo guarda la búsqueda.
            RegisterName::BlackHole | RegisterName::LastInsert | RegisterName::LastSearch => None,
        }
    }

    /// Guarda en `name` y devuelve lo que quedó ahí. Como en nvim, `""`
    /// escribe en `"0`.
    fn store(
        &mut self,
        name: RegisterName,
        register: Register,
        clipboard: &mut dyn Clipboard,
    ) -> Register {
        let slot = match name {
            RegisterName::Named(letter) => &mut self.named[usize::from(letter)],
            RegisterName::Append(letter) => {
                let slot = &mut self.named[usize::from(letter)];
                let appended = match slot.take() {
                    Some(old) => old.append(&register),
                    None => register,
                };
                *slot = Some(appended.clone());
                return appended;
            }
            RegisterName::Unnamed | RegisterName::Yank => &mut self.yank,
            RegisterName::Numbered(n) => &mut self.numbered[usize::from(n - 1)],
            RegisterName::SmallDelete => &mut self.small_delete,
            RegisterName::System(system) => {
                clipboard.write(system, register.to_clipboard());
                return register;
            }
            RegisterName::BlackHole | RegisterName::LastInsert | RegisterName::LastSearch => {
                return register;
            }
        };
        *slot = Some(register.clone());
        register
    }
}
