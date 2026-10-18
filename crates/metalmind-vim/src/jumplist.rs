//! La jumplist de vim: dónde estaba el cursor antes de cada salto, de cada
//! panel y entre archivos. Port de `setpcmark`, `cleanup_jumplist` y
//! `get_jumplist` de `mark.c` de nvim 0.12.6, con `jumpoptions=clean`.

use metalmind_buffer::{Point, Splice};

/// Como el `JUMPLISTSIZE` de vim.
pub const CAPACITY: usize = 100;

/// Un archivo para la jumplist y las marks `A`–`Z`. Lo asigna quien aloja
/// el motor y sobrevive a cerrar el buffer, como el número de buffer de vim.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(u32);

impl FileId {
    #[must_use]
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// Una posición en un archivo: una entrada de la jumplist o una mark de
/// archivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Jump {
    pub file: FileId,
    pub point: Point,
}

/// Un salto que cae en otro archivo (`ctrl-o`, `ctrl-i`, `'A`): quien aloja
/// el motor cambia el buffer del panel y le pasa esto a
/// [`Vim::land`](crate::Vim::land), que pone el cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
    pub file: FileId,
    pub point: Point,
    /// `'A` va al primer no blanco de la línea; `` `A `` y la jumplist, a
    /// la posición.
    pub first_non_blank: bool,
}

/// La jumplist de un panel. `index` es la entrada a la que volvería
/// `ctrl-i`; vale `len` si no se ha retrocedido desde el último salto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JumpList {
    entries: Vec<Jump>,
    index: usize,
}

/// `ctrl-o` va a las entradas más viejas y `ctrl-i` a las más nuevas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Older,
    Newer,
}

/// A dónde lleva `ctrl-o` o `ctrl-i`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    pub target: Jump,
    /// Era el primer `ctrl-o` tras un salto: el cursor se agregó a la lista,
    /// así que también es la mark de `''`.
    pub pushed_here: bool,
}

impl JumpList {
    #[must_use]
    pub fn entries(&self) -> &[Jump] {
        &self.entries
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// `setpcmark`: agrega `jump` al final y deja el índice detrás; si está
    /// llena, se pierde la más vieja.
    pub fn push(&mut self, jump: Jump) {
        if self.entries.len() == CAPACITY {
            self.entries.remove(0);
        }
        self.entries.push(jump);
        self.index = self.entries.len();
    }

    /// `cleanup_jumplist`: de las entradas de un mismo archivo y línea queda
    /// la última, y si el índice está al final, se quita la última entrada
    /// cuando es la línea de `here`.
    pub fn cleanup(&mut self, here: Jump) {
        let mut kept = Vec::with_capacity(self.entries.len());
        let mut index = None;
        for (from, jump) in self.entries.iter().enumerate() {
            if from == self.index {
                index = Some(kept.len());
            }
            let repeated = self.entries[from + 1..]
                .iter()
                .any(|later| same_line(later, jump));
            if !repeated {
                kept.push(*jump);
            }
        }
        self.index = index.unwrap_or(kept.len());
        self.entries = kept;
        let at_end = self.index == self.entries.len();
        if at_end
            && self
                .entries
                .last()
                .is_some_and(|last| same_line(last, &here))
        {
            self.entries.pop();
            self.index -= 1;
        }
    }

    /// `get_jumplist`: la entrada `count` lugares hacia `direction` (un
    /// count de 0 vale 1), saltando las de archivos que ya no existen.
    /// El primer `ctrl-o` tras un salto agrega `here` para poder volver.
    /// `None` si no hay a dónde ir.
    pub(crate) fn step(
        &mut self,
        direction: Direction,
        count: usize,
        here: Jump,
        exists: impl Fn(FileId) -> bool,
    ) -> Option<Step> {
        self.cleanup(here);
        let count = isize::try_from(count.max(1)).ok()?;
        let mut count = match direction {
            Direction::Older => -count,
            Direction::Newer => count,
        };
        let mut pushed_here = false;
        loop {
            self.index
                .checked_add_signed(count)
                .filter(|&next| next < self.entries.len())?;
            if self.index == self.entries.len() {
                self.push(here);
                pushed_here = true;
                self.index -= 1;
            }
            self.index = self.index.checked_add_signed(count)?;
            let target = self.entries[self.index];
            if target.file == here.file || exists(target.file) {
                return Some(Step {
                    target,
                    pushed_here,
                });
            }
            // vim, al saltarse una entrada, avanza cada vez más; aquí se
            // salta solo esa.
            count = count.signum();
        }
    }

    /// Mueve las entradas de `file` con lo que le hicieron `splices`; si se
    /// borra su línea, quedan donde empezó lo borrado, como en vim.
    pub fn adjust(&mut self, file: FileId, splices: &[Splice]) {
        for jump in self.entries.iter_mut().filter(|jump| jump.file == file) {
            jump.point = splices
                .iter()
                .fold(jump.point, |point, splice| splice.lines.keep(point));
        }
    }
}

fn same_line(a: &Jump, b: &Jump) -> bool {
    a.file == b.file && a.point.line == b.point.line
}

#[cfg(test)]
#[path = "jumplist_tests.rs"]
mod tests;
