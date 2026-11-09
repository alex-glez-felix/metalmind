//! Los índices que se leen juntos son de un mismo lote, aunque el watcher
//! publique mientras se leen.

#![allow(
    clippy::unwrap_used,
    reason = "en tests, un unwrap que falla es un test que falla"
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use metalmind_vault::{Vault, VaultPath, VaultSnapshot, write_atomic};
use tempfile::TempDir;

const NOTES: usize = 30;

/// Una nota que deja algo en cada uno de los cinco índices.
const TEXT: &[u8] = b"---\ntitulo: Uno\n---\n# Uno\n[[otra]] #tema\n";

/// En qué índices está `note`: si son del mismo lote, en todos o en ninguno.
fn seen(snapshot: &VaultSnapshot, note: &VaultPath) -> [bool; 5] {
    [
        snapshot.files.kind(note).is_some(),
        !snapshot.links.outgoing(note).is_empty(),
        snapshot
            .tags
            .index()
            .note(note)
            .is_some_and(|tags| !tags.occurrences.is_empty()),
        !snapshot.properties.of(note).is_empty(),
        snapshot.summaries.of(note).is_some(),
    ]
}

fn same_batch(snapshot: &VaultSnapshot, notes: &[VaultPath]) {
    for note in notes {
        let seen = seen(snapshot, note);
        assert!(seen == [true; 5] || seen == [false; 5], "{note}: {seen:?}");
    }
}

/// Para al lector también si el test falla antes: si no, `thread::scope` lo
/// esperaría para siempre.
struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[test]
fn un_lector_ve_los_cinco_indices_del_mismo_lote() {
    let dir = TempDir::new().unwrap();
    let (vault, updates) = Vault::open(dir.path()).unwrap();
    let notes: Vec<VaultPath> = (0..NOTES)
        .map(|n| VaultPath::new(&format!("n{n}.md")).unwrap())
        .collect();
    let done = AtomicBool::new(false);
    thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let mut reads = 0_usize;
            while !done.load(Ordering::Acquire) {
                same_batch(&vault.snapshot(), &notes);
                reads += 1;
            }
            reads
        });
        let stop = StopOnDrop(&done);
        for note in &notes {
            write_atomic(&dir.path().join(note.as_str()), |out| out.write_all(TEXT)).unwrap();
            loop {
                let update = updates.recv_timeout(Duration::from_secs(10)).unwrap();
                same_batch(&update.snapshot, &notes);
                if update.snapshot.files.kind(note).is_some() {
                    break;
                }
            }
        }
        drop(stop);
        assert!(reader.join().unwrap() > 0);
    });
    assert_eq!(seen(&vault.snapshot(), &notes[NOTES - 1]), [true; 5]);
}
