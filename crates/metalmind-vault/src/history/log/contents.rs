//! Recorrer el contenido de varias versiones de un log en orden, sin
//! tenerlas todas en memoria.

use super::{Corrupt, DAMAGED, NoteLog, delta};

/// El contenido de los registros pedidos, uno tras otro. Reconstruye cada
/// registro una sola vez y suelta cada texto en cuanto nada más lo necesita.
pub(in crate::history) struct Contents<'a> {
    log: &'a NoteLog,
    wanted: &'a [usize],
    next: usize,
    built: usize,
    texts: Texts,
}

impl NoteLog {
    pub(in crate::history) fn contents<'a>(
        &'a self,
        wanted: &'a [usize],
    ) -> Result<Contents<'a>, Corrupt> {
        let len = wanted.iter().max().map_or(0, |last| last + 1);
        let mut texts = Texts {
            uses: vec![0; len],
            texts: vec![None; len],
        };
        for index in 0..len {
            if let Some(base) = self.record(index)?.base {
                texts.uses[base] += 1;
            }
        }
        for &index in wanted {
            texts.uses[index] += 1;
        }
        Ok(Contents {
            log: self,
            wanted,
            next: 0,
            built: 0,
            texts,
        })
    }
}

impl Contents<'_> {
    pub(in crate::history) fn next_text(&mut self) -> Result<Option<String>, Corrupt> {
        let Some(&index) = self.wanted.get(self.next) else {
            return Ok(None);
        };
        self.next += 1;
        while self.built <= index {
            let text = self.build(self.built)?;
            self.texts.keep(self.built, text);
            self.built += 1;
        }
        let text = String::from_utf8(self.texts.get(index)?.to_vec()).map_err(|_| DAMAGED)?;
        self.texts.release(index);
        Ok(Some(text))
    }

    fn build(&mut self, index: usize) -> Result<Vec<u8>, Corrupt> {
        let current = self.log.record(index)?;
        let payload = current.open()?;
        match current.base {
            None if payload.len() == current.text_len => Ok(payload),
            None => Err(DAMAGED),
            Some(base) => {
                let bytes = delta::apply(self.texts.get(base)?, &payload, current.text_len)
                    .ok_or(DAMAGED)?;
                self.texts.release(base);
                Ok(bytes)
            }
        }
    }
}

struct Texts {
    uses: Vec<usize>,
    texts: Vec<Option<Vec<u8>>>,
}

impl Texts {
    fn keep(&mut self, index: usize, text: Vec<u8>) {
        if self.uses[index] > 0 {
            self.texts[index] = Some(text);
        }
    }

    fn get(&self, index: usize) -> Result<&[u8], Corrupt> {
        self.texts[index].as_deref().ok_or(DAMAGED)
    }

    fn release(&mut self, index: usize) {
        self.uses[index] -= 1;
        if self.uses[index] == 0 {
            self.texts[index] = None;
        }
    }
}
