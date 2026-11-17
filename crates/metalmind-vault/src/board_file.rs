//! Una pizarra para el vault: lo que los índices sacan de ella y cómo se
//! le aplica una edición. Las dos cosas van sobre su texto
//! ([`metalmind_board::BoardText`]): el formato `.canvas` no sale de
//! `metalmind-board`.

use metalmind_board::{Board, BoardText, Passage, PassageKind};

use crate::edit::{FileEdits, Mismatch, StaleEdit};
use crate::index::EntryKind;
use crate::links::IndexedLink;
use crate::links::note::links_of;
use crate::note_scan::NoteScan;
use crate::path::VaultPath;
use crate::tags::tags_of;

/// Lo escaneado de `file` si `path` es una pizarra: de cada tarjeta, un link
/// a su nota, y del texto de cada caja, sus links y sus `#tags`. Una que no
/// se lee queda sin nada.
pub(crate) fn scan(path: &VaultPath, file: &str) -> Option<NoteScan> {
    if EntryKind::of_file(path) != EntryKind::Board {
        return None;
    }
    let board = match Board::parse(file) {
        Ok(board) => board,
        Err(error) => {
            log::warn!("{path} no se lee como pizarra: {error}");
            return Some(NoteScan::default());
        }
    };
    let text = board.text();
    let mut scan = NoteScan::default();
    let mut line = 0;
    for passage in text.passages() {
        let words = words(&text, passage);
        let start = passage.range.start;
        match &passage.kind {
            PassageKind::Card { subpath } => {
                let joined = passage.joined.iter().map(|&at| words_at(&text, at));
                let card = IndexedLink::card(words, subpath.as_deref(), start, line, joined);
                scan.links.push(card);
            }
            PassageKind::Text => {
                let scanned = metalmind_syntax::scan(words);
                let links = links_of(words, &scanned.links);
                scan.links
                    .extend(links.into_iter().map(|link| link.shifted(start, line)));
                let tags = tags_of(words, &scanned).inline_shifted(start, line);
                scan.tags.occurrences.extend(tags);
            }
        }
        line += words.matches('\n').count() + 1;
    }
    Some(scan)
}

fn words<'t>(text: &'t BoardText, passage: &Passage) -> &'t str {
    &text.as_str()[passage.range.clone()]
}

fn words_at(text: &BoardText, passage: usize) -> &str {
    text.passages()
        .get(passage)
        .map_or("", |passage| words(text, passage))
}

/// El texto de la pizarra `file`, sobre el que van los rangos de sus
/// ediciones.
pub(crate) fn text(edits: &FileEdits, file: &str) -> Result<Option<String>, StaleEdit> {
    Ok(parse(edits, file)?.map(|board| board.text().as_str().to_owned()))
}

/// `file` con `edits` aplicadas a su texto, si es una pizarra: el archivo
/// nuevo y las ediciones que lo vuelven al de antes.
pub(crate) fn apply(
    edits: &FileEdits,
    file: &str,
) -> Result<Option<(String, FileEdits)>, StaleEdit> {
    let Some(mut board) = parse(edits, file)? else {
        return Ok(None);
    };
    let (_, undo) = edits.apply_to(board.text().as_str())?;
    let replaced = edits
        .edits
        .iter()
        .map(|edit| (edit.range.clone(), edit.new.as_str()));
    board
        .replace_text(replaced)
        .map_err(|outside| edits.stale(Mismatch::AcrossNodes(outside.0)))?;
    Ok(Some((board.write(), undo)))
}

/// `Ok(None)` si las ediciones no son de una pizarra.
fn parse(edits: &FileEdits, file: &str) -> Result<Option<Board>, StaleEdit> {
    if EntryKind::of_file(&edits.path) != EntryKind::Board {
        return Ok(None);
    }
    Board::parse(file)
        .map(Some)
        .map_err(|error| edits.stale(Mismatch::NotABoard(error.to_string())))
}
