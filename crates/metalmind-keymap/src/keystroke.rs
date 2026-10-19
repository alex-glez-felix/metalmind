//! Una tecla de un atajo de la ventana, con más teclas y modificadores que
//! las [`Key`] del motor (`home`, `ctrl-shift-p`).

use metalmind_vim::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Keystroke {
    pub ctrl: bool,
    /// Solo con otro modificador o una tecla con nombre: una mayúscula ya
    /// lleva el shift en su carácter, como en vim.
    pub shift: bool,
    pub key: KeyName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyName {
    Char(char),
    Escape,
    Enter,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
}

impl Keystroke {
    #[must_use]
    pub const fn plain(key: KeyName) -> Self {
        Self {
            ctrl: false,
            shift: false,
            key,
        }
    }

    #[must_use]
    pub const fn ctrl(key: KeyName) -> Self {
        Self {
            ctrl: true,
            shift: false,
            key,
        }
    }

    #[must_use]
    pub const fn ctrl_shift(key: KeyName) -> Self {
        Self {
            ctrl: true,
            shift: true,
            key,
        }
    }
}

impl From<Key> for Keystroke {
    fn from(key: Key) -> Self {
        match key {
            Key::Char(c) => Self::plain(KeyName::Char(c)),
            Key::Ctrl(c) => Self::ctrl(KeyName::Char(c)),
            Key::Escape => Self::plain(KeyName::Escape),
            Key::Enter => Self::plain(KeyName::Enter),
            Key::Backspace => Self::plain(KeyName::Backspace),
            Key::Tab => Self::plain(KeyName::Tab),
            Key::Up => Self::plain(KeyName::Up),
            Key::Down => Self::plain(KeyName::Down),
        }
    }
}

impl KeyName {
    /// Como lo escriben GPUI y Zed: `space`, `escape`, `g`.
    #[must_use]
    pub fn label(self) -> std::borrow::Cow<'static, str> {
        let name = match self {
            Self::Char(' ') => "space",
            Self::Char(c) => return c.to_string().into(),
            Self::Escape => "escape",
            Self::Enter => "enter",
            Self::Backspace => "backspace",
            Self::Tab => "tab",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Home => "home",
            Self::End => "end",
            Self::Delete => "delete",
        };
        name.into()
    }
}

/// Como las [`Key`] del motor: `ctrl-w`, `space`, `G`, `ctrl-shift-p`.
impl std::fmt::Display for Keystroke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctrl {
            f.write_str("ctrl-")?;
        }
        if self.shift {
            f.write_str("shift-")?;
        }
        f.write_str(&self.key.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La paleta muestra igual una tecla del motor y una de la ventana.
    #[test]
    fn se_escribe_como_la_tecla_del_motor() {
        let keys = [
            Key::Char('g'),
            Key::Char('G'),
            Key::Char(' '),
            Key::Ctrl('w'),
            Key::Escape,
            Key::Enter,
            Key::Backspace,
            Key::Tab,
            Key::Up,
            Key::Down,
        ];
        for key in keys {
            assert_eq!(Keystroke::from(key).to_string(), key.to_string());
        }
        let palette = Keystroke::ctrl_shift(KeyName::Char('p'));
        assert_eq!(palette.to_string(), "ctrl-shift-p");
        assert_eq!(
            Keystroke::ctrl(KeyName::Backspace).to_string(),
            "ctrl-backspace"
        );
    }
}
