//! Quien guarda las notas recientes: un hilo propio que escribe en orden y
//! se salta las versiones que ya quedaron viejas. Al soltarlo se espera a
//! que termine lo pendiente, como el watcher de `nota-vault`: cerrar la
//! ventana o cambiar de vault no pierde la última escritura.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

pub struct RecentsWriter {
    texts: Option<mpsc::Sender<String>>,
    thread: Option<JoinHandle<()>>,
}

impl RecentsWriter {
    /// # Errors
    /// Si no se puede arrancar el hilo.
    pub fn spawn(file: PathBuf) -> io::Result<Self> {
        let (texts, pending) = mpsc::channel::<String>();
        let thread = thread::Builder::new()
            .name("nota-ui recientes".to_owned())
            .spawn(move || {
                while let Ok(mut text) = pending.recv() {
                    // Solo importa la última.
                    while let Ok(newer) = pending.try_recv() {
                        text = newer;
                    }
                    if let Err(error) = write_recents(&file, &text) {
                        log::warn!("no se guardaron las notas recientes: {error}");
                    }
                }
            })?;
        Ok(Self {
            texts: Some(texts),
            thread: Some(thread),
        })
    }

    pub fn write(&self, text: String) {
        if let Some(texts) = &self.texts
            && texts.send(text).is_err()
        {
            log::warn!("el hilo de las notas recientes ya no está");
        }
    }
}

impl Drop for RecentsWriter {
    fn drop(&mut self) {
        // Sin emisor, el hilo escribe lo que quede y sale.
        self.texts.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("el hilo de las notas recientes terminó en pánico");
        }
    }
}

pub fn write_recents(file: &Path, text: &str) -> Result<(), nota_vault::FileError> {
    if let Some(folder) = file.parent()
        && let Err(source) = fs::create_dir_all(folder)
    {
        return Err(nota_vault::FileError::Write {
            path: folder.to_owned(),
            source,
        });
    }
    nota_vault::write_atomic(file, |out| out.write_all(text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Soltarlo justo después de escribir no pierde nada: se espera al hilo.
    #[test]
    fn al_soltarlo_queda_escrita_la_ultima() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state/vaults/x/recientes");
        let writer = RecentsWriter::spawn(file.clone()).unwrap();
        for n in 0..50 {
            writer.write(format!("{n}.md\n"));
        }
        drop(writer);
        assert_eq!(fs::read_to_string(&file).unwrap(), "49.md\n");
    }
}
