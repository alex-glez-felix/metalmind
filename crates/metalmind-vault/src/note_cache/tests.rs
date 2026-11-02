use std::collections::HashMap;
use std::sync::Mutex;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::format::{self, Unusable};
use super::*;
use crate::index::EntryKind;
use crate::tags::NoteTags;

const OLD: Stamp = Stamp {
    size: 10,
    modified_seconds: 1_000,
    modified_nanos: 5,
};

fn path(text: &str) -> VaultPath {
    VaultPath::new(text).unwrap()
}

/// Un vault en memoria que cuenta cuántas veces se lee cada nota.
struct Disk {
    notes: HashMap<VaultPath, (Option<Stamp>, Option<String>)>,
    reads: Mutex<Vec<VaultPath>>,
}

impl Disk {
    fn new(notes: &[(&str, Option<Stamp>, &str)]) -> Self {
        Self {
            notes: notes
                .iter()
                .map(|(name, stamp, text)| (path(name), (*stamp, Some((*text).to_owned()))))
                .collect(),
            reads: Mutex::default(),
        }
    }

    fn paths(&self) -> Vec<VaultPath> {
        let mut paths: Vec<VaultPath> = self.notes.keys().cloned().collect();
        paths.sort_unstable();
        paths
    }

    fn open(&self, file: Option<&Path>) -> OpenedNotes {
        open_notes(
            file,
            self.paths(),
            &|note: &VaultPath| self.notes.get(note).and_then(|(stamp, _)| *stamp),
            &|note: &VaultPath| {
                self.reads.lock().unwrap().push(note.clone());
                self.notes.get(note).and_then(|(_, text)| text.clone())
            },
        )
    }

    /// Abre y escribe la caché como lo hace el hilo del vault.
    fn open_and_save(&self, file: &Path) -> OpenedNotes {
        let opened = self.open(Some(file));
        if let Some(refresh) = &opened.refresh {
            let (links, tags) = indexes(&opened);
            refresh.write(&links, &tags);
        }
        opened
    }

    fn read_count(&self) -> usize {
        self.reads.lock().unwrap().len()
    }
}

fn indexes(opened: &OpenedNotes) -> (LinkIndex, Tags) {
    let links = LinkIndex::from_scans(
        opened
            .scans
            .iter()
            .map(|(path, _)| (path.clone(), EntryKind::Note)),
        opened
            .scans
            .iter()
            .map(|(path, scan)| (path.clone(), scan.links.clone()))
            .collect(),
    );
    let tags = Tags::from_scans(
        VaultPath::fixed(crate::tags::DEFAULT_TAGS_FOLDER),
        opened
            .scans
            .iter()
            .map(|(path, scan)| (path.clone(), scan.tags.clone(), scan.properties.clone()))
            .collect(),
        |_: &VaultPath| None,
    );
    (links, tags)
}

fn scan_of<'o>(opened: &'o OpenedNotes, name: &str) -> &'o NoteScan {
    let (_, scan) = opened
        .scans
        .iter()
        .find(|(path, _)| path.as_str() == name)
        .unwrap();
    scan
}

fn targets(scan: &NoteScan) -> Vec<&str> {
    scan.links
        .iter()
        .map(|link| link.target_text.as_str())
        .collect()
}

#[test]
fn con_el_mismo_sello_no_se_lee_la_nota() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cache");
    let disk = Disk::new(&[
        ("a.md", Some(OLD), "[[b]] #uno"),
        ("b.md", Some(OLD), "hola"),
    ]);
    disk.open_and_save(&file);
    assert_eq!(disk.read_count(), 2);

    let disk = Disk::new(&[
        ("a.md", Some(OLD), "OTRO TEXTO"),
        ("b.md", Some(OLD), "hola"),
    ]);
    let opened = disk.open_and_save(&file);
    assert_eq!(disk.read_count(), 0);
    assert_eq!(targets(scan_of(&opened, "a.md")), ["b"]);
    assert!(opened.refresh.is_none(), "la caché ya estaba al día");
}

#[test]
fn con_otro_sello_se_lee_y_se_escanea() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cache");
    Disk::new(&[("a.md", Some(OLD), "[[b]]")]).open_and_save(&file);

    let newer = Stamp {
        modified_nanos: 6,
        ..OLD
    };
    let bigger = Stamp { size: 11, ..OLD };
    for stamp in [newer, bigger] {
        let disk = Disk::new(&[("a.md", Some(stamp), "[[c]]")]);
        let opened = disk.open(Some(&file));
        assert_eq!(disk.read_count(), 1);
        assert_eq!(targets(scan_of(&opened, "a.md")), ["c"]);
        assert!(opened.refresh.is_some());
    }
}

#[test]
fn un_touch_no_vuelve_a_escanear() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cache");
    let text = "[[b]]";
    save_without_links(&file, "a.md", OLD, ContentHash::of(text));

    let touched = Stamp {
        modified_seconds: 2_000,
        ..OLD
    };
    let disk = Disk::new(&[("a.md", Some(touched), text)]);
    let opened = disk.open(Some(&file));
    assert_eq!(disk.read_count(), 1, "hay que leerla para saber su hash");
    assert_eq!(
        targets(scan_of(&opened, "a.md")),
        Vec::<&str>::new(),
        "lo escaneado sale de la caché, que aquí no tiene links"
    );
    assert!(opened.refresh.is_some(), "el sello nuevo hay que guardarlo");
}

/// Una caché con `name` sin links, sea lo que sea que diga su texto.
fn save_without_links(file: &Path, name: &str, stamp: Stamp, hash: ContentHash) {
    let tags = NoteTags::default();
    let record = NoteRecord {
        path: Cow::Borrowed(name),
        stamp: Some(stamp),
        hash,
        links: Cow::Borrowed(&[]),
        tags: Cow::Borrowed(&tags),
        properties: Cow::Borrowed(&[]),
    };
    format::save(file, &[record]).unwrap();
}

#[test]
fn un_mtime_reciente_no_es_de_fiar() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cache");
    let recent = Stamp {
        modified_seconds: now_seconds(),
        ..OLD
    };
    let disk = Disk::new(&[("a.md", Some(recent), "[[b]]")]);
    let opened = disk.open_and_save(&file);
    assert_eq!(targets(scan_of(&opened, "a.md")), ["b"]);

    // Otra escritura en el mismo instante y del mismo tamaño: el sello no cambia.
    let disk = Disk::new(&[("a.md", Some(recent), "[[c]]")]);
    let opened = disk.open(Some(&file));
    assert_eq!(disk.read_count(), 1);
    assert_eq!(targets(scan_of(&opened, "a.md")), ["c"]);
}

#[test]
fn un_mtime_se_asienta_a_los_dos_segundos() {
    let settled = Stamp {
        modified_seconds: 0,
        ..OLD
    };
    assert!(settled.is_settled(SETTLE_SECONDS));
    assert!(!settled.is_settled(SETTLE_SECONDS - 1));
}

#[test]
fn una_nota_ilegible_queda_vacia_y_se_reintenta() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cache");
    let mut disk = Disk::new(&[("a.md", Some(OLD), "[[b]]")]);
    disk.notes.get_mut(&path("a.md")).unwrap().1 = None;
    let opened = disk.open_and_save(&file);
    assert_eq!(scan_of(&opened, "a.md"), &NoteScan::default());

    let disk = Disk::new(&[("a.md", Some(OLD), "[[b]]")]);
    let opened = disk.open(Some(&file));
    assert_eq!(disk.read_count(), 1);
    assert_eq!(targets(scan_of(&opened, "a.md")), ["b"]);
}

#[test]
fn una_nota_borrada_sale_de_la_cache() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cache");
    Disk::new(&[("a.md", Some(OLD), "x"), ("b.md", Some(OLD), "y")]).open_and_save(&file);

    let disk = Disk::new(&[("b.md", Some(OLD), "y")]);
    let opened = disk.open_and_save(&file);
    assert_eq!(disk.read_count(), 0);
    assert!(opened.refresh.is_some(), "hay que olvidar a a.md");
    assert_eq!(format::load(&file).keys().collect::<Vec<_>>(), ["b.md"]);
}

/// Una caché válida de varios bloques, para romperla de varias formas.
fn valid_cache(dir: &TempDir) -> (PathBuf, Disk) {
    let file = dir.path().join("cache");
    let names: Vec<String> = (0..2_500).map(|n| format!("nota-{n}.md")).collect();
    let notes: Vec<(&str, Option<Stamp>, &str)> = names
        .iter()
        .map(|name| (name.as_str(), Some(OLD), "---\ntags: [a]\n---\n[[b]] #c"))
        .collect();
    let disk = Disk::new(&notes);
    disk.open_and_save(&file);
    (file, Disk::new(&notes))
}

/// Con la caché como esté en `file`, se lee todo otra vez y queda sana.
fn rebuilds(file: &Path, disk: &Disk) {
    let opened = disk.open_and_save(file);
    assert_eq!(disk.read_count(), disk.notes.len());
    assert_eq!(targets(scan_of(&opened, "nota-0.md")), ["b"]);
    assert_eq!(format::load(file).len(), disk.notes.len());
}

#[test]
fn una_cache_ausente_se_construye() {
    let dir = TempDir::new().unwrap();
    let (file, disk) = valid_cache(&dir);
    fs::remove_file(&file).unwrap();
    assert!(format::load(&file).is_empty());
    rebuilds(&file, &disk);
}

#[test]
fn una_carpeta_que_falta_se_crea() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("no/existe/cache");
    let disk = Disk::new(&[("a.md", Some(OLD), "x")]);
    disk.open_and_save(&file);
    assert_eq!(format::load(&file).len(), 1);
}

#[test]
fn cualquier_byte_cambiado_la_descarta() {
    let dir = TempDir::new().unwrap();
    let (file, disk) = valid_cache(&dir);
    let good = fs::read(&file).unwrap();
    assert_eq!(format::parse(&good).unwrap().len(), disk.notes.len());
    let positions = (0..good.len()).step_by(good.len() / 500);
    for at in positions.chain([good.len() - 1]) {
        let mut bad = good.clone();
        bad[at] ^= 0x40;
        assert!(format::parse(&bad).is_err(), "byte {at}");
    }
    let mut bad = good;
    let middle = bad.len() / 2;
    bad[middle] ^= 1;
    fs::write(&file, bad).unwrap();
    assert!(format::load(&file).is_empty());
    rebuilds(&file, &disk);
}

#[test]
fn una_cache_cortada_o_con_basura_se_descarta() {
    let dir = TempDir::new().unwrap();
    let (file, disk) = valid_cache(&dir);
    let good = fs::read(&file).unwrap();
    for len in [0, 4, 8, 40, 44, 60, good.len() / 2, good.len() - 1] {
        assert!(format::parse(&good[..len]).is_err(), "cortada en {len}");
    }
    let mut longer = good.clone();
    longer.push(0);
    assert_eq!(format::parse(&longer).err(), Some(Unusable::Corrupt));
    assert_eq!(
        format::parse(b"no soy una cache, soy un texto cualquiera").err(),
        Some(Unusable::NotACache)
    );
    fs::write(&file, b"basura").unwrap();
    rebuilds(&file, &disk);
}

#[test]
fn una_cache_de_otra_version_se_descarta() {
    let dir = TempDir::new().unwrap();
    let (file, disk) = valid_cache(&dir);
    let mut other = fs::read(&file).unwrap();
    let version_at = other
        .windows(format::version().len())
        .position(|window| window == format::version())
        .unwrap();
    other[version_at] ^= 1;
    assert_eq!(format::parse(&other).err(), Some(Unusable::OtherVersion));
    fs::write(&file, other).unwrap();
    assert!(format::load(&file).is_empty());
    rebuilds(&file, &disk);
}

#[test]
fn un_archivo_que_no_se_puede_escribir_no_molesta() {
    let dir = TempDir::new().unwrap();
    let blocked = dir.path().join("archivo");
    fs::write(&blocked, "").unwrap();
    let disk = Disk::new(&[("a.md", Some(OLD), "[[b]]")]);
    let opened = disk.open_and_save(&blocked.join("cache"));
    assert_eq!(targets(scan_of(&opened, "a.md")), ["b"]);
}

#[test]
fn lo_que_vuelve_de_la_cache_es_igual_a_escanear() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("cache");
    let text = "---\ntags: [uno, \"tr\\u0065s\"]\nfecha: {a: [1, 2]}\n---\n\
                [[b#Sec|x]] ![[c.png]] [d](<mi nota.md#^id>) #cuatro\n";
    Disk::new(&[("a.md", Some(OLD), text)]).open_and_save(&file);
    let disk = Disk::new(&[("a.md", Some(OLD), "otro")]);
    let opened = disk.open(Some(&file));
    assert_eq!(disk.read_count(), 0);
    assert_eq!(scan_of(&opened, "a.md"), &NoteScan::of(text));
}

#[test]
fn el_sello_sale_del_archivo() {
    let dir = TempDir::new().unwrap();
    let note = dir.path().join("a.md");
    fs::write(&note, "hola").unwrap();
    let stamp = Stamp::of_file(&note).unwrap();
    assert_eq!(stamp.size, 4);
    assert_eq!(Stamp::of_file(&dir.path().join("no.md")), None);
}
