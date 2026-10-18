use pretty_assertions::assert_eq;

use super::*;

fn write_all(versions: &[&str]) -> (NoteLog, Vec<u8>) {
    let mut log = NoteLog::default();
    let mut file = Vec::new();
    for (at, text) in (0..).zip(versions) {
        if let Some((_, frames)) = log.append(text, at, None).unwrap() {
            file.extend(frames);
        }
    }
    (log, file)
}

#[test]
fn lo_escrito_se_vuelve_a_leer_igual() {
    let (log, file) = write_all(&["uno\n", "uno\ndos\n", "", "uno\ndos\ntres"]);
    let read = NoteLog::parse(&file).unwrap();
    assert_eq!(read.entries(), log.entries());
    assert_eq!(read.valid_len(), file.len() as u64);
    let texts: Vec<String> = read
        .entries()
        .iter()
        .map(|e| read.content(e.record).unwrap())
        .collect();
    assert_eq!(texts, ["uno\n", "uno\ndos\n", "", "uno\ndos\ntres"]);
}

#[test]
fn un_contenido_repetido_no_ocupa_otra_vez() {
    let (log, _) = write_all(&["a\n", "a\n", "b\n", "a\n"]);
    let records: Vec<usize> = log.entries().iter().map(|e| e.record).collect();
    assert_eq!(
        records,
        [0, 1, 0],
        "el segundo `a` seguido no es versión; el tercero comparte"
    );
    assert_eq!(log.records.len(), 2);
}

#[test]
fn deltas_hasta_que_pesan_lo_que_un_snapshot() {
    let lines: Vec<String> = (0..400)
        .map(|n| format!("línea {n} de una nota larga\n"))
        .collect();
    let versions: Vec<String> = (0..300)
        .map(|n| {
            let mut lines = lines.clone();
            lines[n % 400] = format!("la línea {n} cambió {}\n", "x".repeat(n % 50));
            lines.concat()
        })
        .collect();
    let refs: Vec<&str> = versions.iter().map(String::as_str).collect();
    let (log, file) = write_all(&refs);
    let snapshots = log.records.iter().filter(|r| r.base.is_none()).count();
    assert!(
        snapshots > 1,
        "la cadena se corta al pesar lo que su snapshot"
    );
    for record in &log.records {
        assert!(record.chain_bytes <= log.records[record.root].stored.len());
    }
    let raw: usize = versions.iter().map(String::len).sum();
    assert!(file.len() < raw / 20, "{} de {raw} bytes", file.len());
    for (entry, text) in log.entries().iter().zip(&versions) {
        assert_eq!(&log.content(entry.record).unwrap(), text);
    }
}

#[test]
fn una_cola_cortada_se_ignora() {
    let (_, file) = write_all(&["uno\n", "dos\n"]);
    let (whole, _) = write_all(&["uno\n"]);
    for cut in 1..file.len() {
        let log = NoteLog::parse(&file[..cut]).unwrap();
        assert!(log.entries().len() <= 2);
        assert!(log.valid_len() <= cut as u64);
        if cut as u64 >= whole.valid_len() {
            assert!(!log.entries().is_empty(), "corte en {cut}");
        }
    }
    assert!(NoteLog::parse(b"otra cosa").is_err());
}

#[test]
fn recorrer_todas_las_versiones_da_cada_contenido() {
    let versions = ["a\n", "a\nb\n", "c\n", "a\n", "a\nb\nc\n"];
    let (log, _) = write_all(&versions);
    let records: Vec<usize> = log.entries().iter().map(|e| e.record).collect();
    let mut seen = Vec::new();
    log.for_each_content(&records, |text| {
        seen.push(text.to_owned());
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, versions);
}

#[test]
fn una_cola_en_ceros_tambien_es_un_guardado_cortado() {
    let (_, mut file) = write_all(&["uno\n", "dos\n"]);
    file.extend([0; 40]);
    let log = NoteLog::parse(&file).unwrap();
    assert_eq!(log.entries().len(), 2);
}

#[test]
fn un_marco_danado_en_medio_no_se_toma_por_cola_cortada() {
    let (_, mut file) = write_all(&["uno\n", "dos\n", "tres\n"]);
    let (first, _) = write_all(&["uno\n"]);
    let last_of_first = usize::try_from(first.valid_len()).unwrap() - 1;
    file[last_of_first] ^= 0xff;
    assert!(NoteLog::parse(&file).is_err());
}

/// Un marco `VERSION` bien armado, con lo que diga el test.
fn version_frame(seq: u64, len: usize, record: usize) -> Vec<u8> {
    version_frame_of("uno\n", seq, len, record)
}

/// Igual, pero con el hash de `text`.
fn version_frame_of(text: &str, seq: u64, len: usize, record: usize) -> Vec<u8> {
    let mut payload = Vec::new();
    put_var(&mut payload, seq);
    payload.extend_from_slice(&0_i64.to_le_bytes());
    payload.extend_from_slice(ContentHash::of(text).as_bytes());
    put_var(&mut payload, len as u64);
    put_var(&mut payload, record as u64);
    let mut frame = Vec::new();
    push_frame(&mut frame, VERSION, &payload);
    frame
}

#[test]
fn una_version_que_no_cuadra_con_su_registro_es_un_log_danado() {
    let (_, file) = write_all(&["uno\n"]);
    let with = |frame: Vec<u8>| [file.clone(), frame].concat();
    assert!(NoteLog::parse(&with(version_frame(1, 4, 0))).is_ok());
    assert!(
        NoteLog::parse(&with(version_frame(1, 99, 0))).is_err(),
        "largo"
    );
    assert!(
        NoteLog::parse(&with(version_frame(0, 4, 0))).is_err(),
        "número repetido"
    );
    assert!(
        NoteLog::parse(&with(version_frame(u64::MAX, 4, 0))).is_err(),
        "sin siguiente"
    );
}

#[test]
fn una_version_cuyo_hash_no_es_el_de_su_texto_no_se_lee() {
    let (_, file) = write_all(&["uno\n"]);
    let file = [file, version_frame_of("dos\n", 1, 4, 0)].concat();
    let log = NoteLog::parse(&file).unwrap();
    let [good, bad] = log.entries() else {
        panic!("{:?}", log.entries())
    };
    assert_eq!(log.version_text(good).unwrap(), "uno\n");
    assert!(log.version_text(bad).is_err());
}
