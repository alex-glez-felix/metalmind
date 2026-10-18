/// Una tecla ya interpretada por la capa de entrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Escape,
    Enter,
    Backspace,
    Tab,
    Up,
    Down,
}
