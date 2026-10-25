use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;

fn path(text: &str) -> VaultPath {
    VaultPath::new(text).unwrap()
}

fn at(text: &str, line: usize, column: usize) -> SavedJump {
    SavedJump {
        path: path(text),
        line,
        column,
    }
}

/// Un vault con estas notas, y el archivo de los saltos fuera de él.
struct Fixture {
    vault: TempDir,
    state: TempDir,
}

impl Fixture {
    fn new(notes: &[(&str, &str)]) -> Self {
        let fixture = Self {
            vault: TempDir::new().unwrap(),
            state: TempDir::new().unwrap(),
        };
        for (name, text) in notes {
            fixture.write(name, text);
        }
        fixture
    }

    fn root(&self) -> &Path {
        self.vault.path()
    }

    fn file(&self) -> PathBuf {
        self.state.path().join("vaults/notas-1/saltos")
    }

    fn write(&self, name: &str, text: &str) {
        let full = self.root().join(name);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }

    /// Lo que hay en el vault ahora, como lo daría el walk.
    fn index(&self) -> VaultIndex {
        let mut entries = Vec::new();
        for entry in walkdir(self.root()) {
            let relative = entry.strip_prefix(self.root()).unwrap().to_str().unwrap();
            let kind = if entry.is_dir() {
                EntryKind::Folder
            } else {
                EntryKind::of_file(&path(relative))
            };
            entries.push((path(relative), kind));
        }
        VaultIndex::from_entries(entries)
    }

    fn save(&self, saved: &SavedJumps) {
        saved.save(self.root(), &self.file()).unwrap();
    }

    fn load(&self) -> SavedJumps {
        SavedJumps::load(self.root(), &self.file(), &self.index())
    }
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap().path();
        if entry.is_dir() {
            found.extend(walkdir(&entry));
        }
        found.push(entry);
    }
    found
}

fn sample() -> SavedJumps {
    SavedJumps {
        jumps: vec![at("a.md", 0, 0), at("d/b.md", 12, 3), at("a.md", 4, 1)],
        marks: vec![('A', at("d/b.md", 2, 0)), ('Q', at("a.md", 7, 5))],
    }
}

#[test]
fn sin_archivo_no_hay_saltos() {
    let fixture = Fixture::new(&[("a.md", "uno")]);
    assert_eq!(fixture.load(), SavedJumps::default());
}

#[test]
fn lo_guardado_vuelve_igual() {
    let fixture = Fixture::new(&[("a.md", "uno\n"), ("d/b.md", "dos\n")]);
    fixture.save(&sample());
    assert_eq!(fixture.load(), sample());
}

#[test]
fn guardar_de_nuevo_reemplaza_todo_sin_dejar_temporales() {
    let fixture = Fixture::new(&[("a.md", "uno\n"), ("d/b.md", "dos\n")]);
    fixture.save(&sample());
    let later = SavedJumps {
        jumps: vec![at("a.md", 1, 0)],
        marks: Vec::new(),
    };
    fixture.save(&later);
    assert_eq!(fixture.load(), later);
    let dir: Vec<_> = fs::read_dir(fixture.file().parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(dir, ["saltos"]);
}

#[test]
fn una_nota_borrada_pierde_lo_suyo_y_lo_demas_queda() {
    let fixture = Fixture::new(&[("a.md", "uno\n"), ("d/b.md", "dos\n")]);
    fixture.save(&sample());
    fs::remove_file(fixture.root().join("d/b.md")).unwrap();
    assert_eq!(
        fixture.load(),
        SavedJumps {
            jumps: vec![at("a.md", 0, 0), at("a.md", 4, 1)],
            marks: vec![('Q', at("a.md", 7, 5))],
        }
    );
}

#[test]
fn sigue_a_la_nota_renombrada_con_metalmind_cerrado() {
    let fixture = Fixture::new(&[("a.md", "uno\n"), ("d/b.md", "dos\n")]);
    fixture.save(&sample());
    fs::rename(
        fixture.root().join("d/b.md"),
        fixture.root().join("movida.md"),
    )
    .unwrap();
    assert_eq!(
        fixture.load(),
        SavedJumps {
            jumps: vec![at("a.md", 0, 0), at("movida.md", 12, 3), at("a.md", 4, 1)],
            marks: vec![('A', at("movida.md", 2, 0)), ('Q', at("a.md", 7, 5))],
        }
    );
}

/// Si el contenido cambió además del nombre, o hay dos notas con ese
/// contenido, nada dice a dónde fue: se pierde, como si se hubiera borrado.
#[test]
fn un_renombre_dudoso_se_descarta() {
    let fixture = Fixture::new(&[("a.md", "uno\n"), ("d/b.md", "dos\n")]);
    fixture.save(&sample());
    fs::remove_file(fixture.root().join("d/b.md")).unwrap();
    fixture.write("copia-1.md", "dos\n");
    fixture.write("copia-2.md", "dos\n");
    fixture.write("a.md", "uno cambiado\n");
    let loaded = fixture.load();
    assert_eq!(loaded.jumps, [at("a.md", 0, 0), at("a.md", 4, 1)]);
    assert_eq!(loaded.marks, [('Q', at("a.md", 7, 5))]);
}

#[test]
fn una_nota_que_no_se_leyo_al_guardar_no_se_sigue_pero_tampoco_rompe() {
    let fixture = Fixture::new(&[("a.md", "uno\n")]);
    let saved = SavedJumps {
        jumps: vec![at("nunca.md", 1, 1), at("a.md", 0, 0)],
        marks: Vec::new(),
    };
    fixture.save(&saved);
    fixture.write("nunca-bis.md", "");
    assert_eq!(fixture.load().jumps, [at("a.md", 0, 0)]);
}

#[test]
fn una_ruta_con_salto_de_linea_no_se_guarda() {
    let fixture = Fixture::new(&[("a.md", "uno\n"), ("x\ny.md", "raro\n")]);
    let saved = SavedJumps {
        jumps: vec![at("x\ny.md", 0, 0), at("a.md", 0, 0)],
        marks: vec![('B', at("x\ny.md", 0, 0))],
    };
    fixture.save(&saved);
    assert_eq!(
        fixture.load(),
        SavedJumps {
            jumps: vec![at("a.md", 0, 0)],
            marks: Vec::new(),
        }
    );
}

#[test]
fn las_rutas_con_espacios_y_tabs_se_leen_enteras() {
    let fixture = Fixture::new(&[("mis notas/una\tidea .md", "uno\n")]);
    let saved = SavedJumps {
        jumps: vec![at("mis notas/una\tidea .md", 3, 2)],
        marks: vec![('Z', at("mis notas/una\tidea .md", 1, 0))],
    };
    fixture.save(&saved);
    assert_eq!(fixture.load(), saved);
}

#[test]
fn otro_formato_se_ignora() {
    let fixture = Fixture::new(&[("a.md", "uno\n")]);
    fs::create_dir_all(fixture.file().parent().unwrap()).unwrap();
    fs::write(fixture.file(), "metalmind-saltos 2\nsalto 0 0 a.md\n").unwrap();
    assert_eq!(fixture.load(), SavedJumps::default());
    fs::write(fixture.file(), "\u{0}\u{1}basura").unwrap();
    assert_eq!(fixture.load(), SavedJumps::default());
}

#[test]
fn las_lineas_rotas_se_ignoran_y_de_una_mark_repetida_vale_la_ultima() {
    let fixture = Fixture::new(&[("a.md", "uno\n")]);
    fs::create_dir_all(fixture.file().parent().unwrap()).unwrap();
    let text = [
        HEADER,
        "salto x 0 a.md",
        "salto 1 2",
        "salto 1 2 ../fuera.md",
        "salto 1 2 a.md",
        "mark a 0 0 a.md",
        "mark AB 0 0 a.md",
        "mark C 0 0 a.md",
        "mark C 5 1 a.md",
        "nota zz 00 a.md",
        "otra cosa",
        "",
    ]
    .join("\n");
    fs::write(fixture.file(), text).unwrap();
    assert_eq!(
        fixture.load(),
        SavedJumps {
            jumps: vec![at("a.md", 1, 2)],
            marks: vec![('C', at("a.md", 5, 1))],
        }
    );
}

#[test]
fn una_carpeta_en_la_ruta_de_una_nota_no_cuenta_como_la_nota() {
    let fixture = Fixture::new(&[("a.md", "uno\n")]);
    fixture.save(&SavedJumps {
        jumps: vec![at("a.md", 0, 0)],
        marks: Vec::new(),
    });
    fs::remove_file(fixture.root().join("a.md")).unwrap();
    fs::create_dir(fixture.root().join("a.md")).unwrap();
    assert_eq!(fixture.load(), SavedJumps::default());
}
