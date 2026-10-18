//! Renombrar y fusionar como planes (05 §5): cada archivo cambia solo en los
//! rangos esperados, y aplicar el plan da un vault coherente.

use pretty_assertions::assert_eq;

use super::{Fixture, applied, changed, key, owned, path};
use crate::changes::Rename;
use crate::tags::{PlanError, PlanNotice};

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tags/rust.md",
            "---\ncolor: naranja\naliases: [rs]\nparents: [lenguajes]\n---\nNotas de #rust.\n",
        ),
        ("tags/gpui.md", "---\nparents: [rust, ui]\n---\n"),
        ("tags/tokio.md", "---\nparents:\n  - Rust\n  - async\n---\n"),
        ("tags/raro.md", "---\naliases: [rust]\n---\n"),
        (
            "a.md",
            "---\ntitle: Algo\ntags: [Rust, nota]  # comentario\n---\nHoy #rust y #RUST, también #rs.\n",
        ),
        ("b.md", "---\ntags:\n  - \"#rust\"\n  - otra\n---\n"),
        ("c.md", "---\ntags: [\"r\\u0075st\"]\n---\n"),
        ("d.md", "Nada de tags.\n"),
    ])
}

#[test]
fn rename_a_tag_with_page_parents_and_aliases() {
    let fixture = fixture();
    let plan = fixture.tags().plan_rename("#rust", "#Oxido").unwrap();
    assert_eq!(plan.notices, []);
    assert_eq!(
        plan.changes.renames,
        [Rename {
            from: path("tags/rust.md"),
            to: path("tags/Oxido.md"),
        }]
    );
    assert_eq!(
        changed(&fixture, &plan),
        owned(&[
            (
                "a.md",
                "---\ntitle: Algo\ntags: [Oxido, nota]  # comentario\n---\nHoy #Oxido y #Oxido, también #rs.\n"
            ),
            ("b.md", "---\ntags:\n  - \"#Oxido\"\n  - otra\n---\n"),
            ("c.md", "---\ntags: [\"Oxido\"]\n---\n"),
            (
                "tags/Oxido.md",
                "---\ncolor: naranja\naliases: [rs]\nparents: [lenguajes]\n---\nNotas de #Oxido.\n"
            ),
            ("tags/gpui.md", "---\nparents: [Oxido, ui]\n---\n"),
            ("tags/raro.md", "---\naliases: [Oxido]\n---\n"),
            (
                "tags/tokio.md",
                "---\nparents:\n  - Oxido\n  - async\n---\n"
            ),
        ])
    );
    let after = Fixture {
        files: applied(&fixture, &plan),
    }
    .tags();
    assert_eq!(
        after.graph().parents(&key("gpui")),
        [key("oxido"), key("ui")]
    );
    assert_eq!(after.graph().resolve(&key("rs")), &key("oxido"));
    assert_eq!(after.count(&key("oxido")), 3);
    assert!(!after.exists(&key("rust")));
}

#[test]
fn the_original_files_are_kept_for_undo() {
    let fixture = fixture();
    let plan = fixture.tags().plan_rename("#rust", "#Oxido").unwrap();
    for edit in &plan.changes.edits {
        let original = &fixture.files[&edit.path];
        let new = edit.apply(original).unwrap();
        assert_ne!(&new, original);
        assert!(
            edit.apply(&new).is_err(),
            "{}: un plan no se aplica dos veces",
            edit.path
        );
    }
}

#[test]
fn renaming_only_the_case_unifies_spellings() {
    let fixture = fixture();
    let plan = fixture.tags().plan_rename("#rust", "#Rust").unwrap();
    assert_eq!(
        plan.changes.renames,
        [Rename {
            from: path("tags/rust.md"),
            to: path("tags/Rust.md"),
        }]
    );
    let files = applied(&fixture, &plan);
    assert_eq!(
        files[&path("a.md")],
        "---\ntitle: Algo\ntags: [Rust, nota]  # comentario\n---\nHoy #Rust y #Rust, también #rs.\n"
    );
}

#[test]
fn renaming_onto_an_existing_tag_merges() {
    let fixture = fixture();
    let plan = fixture.tags().plan_rename("#nota", "#otra").unwrap();
    assert_eq!(plan.notices, [PlanNotice::Merges { into: key("otra") }]);
    assert!(!plan.is_blocked());
}

#[test]
fn merging_two_tags_with_pages_is_blocked() {
    let plan = fixture().tags().plan_merge("#gpui", "#rust").unwrap();
    assert_eq!(
        plan.notices,
        [
            PlanNotice::Merges { into: key("rust") },
            PlanNotice::BothHavePages {
                from: path("tags/gpui.md"),
                into: path("tags/rust.md"),
            },
        ]
    );
    assert!(plan.is_blocked());
}

#[test]
fn merging_removes_duplicates_from_tags_lists() {
    let fixture = Fixture::new(&[
        ("tags/tokio.md", "---\nparents: [async, nota]\n---\n"),
        ("tags/nota.md", "---\nparents: [tokio]\n---\n"),
        (
            "a.md",
            "---\ntags: [tokio, Nota, tokio]\n---\n#tokio #nota\n",
        ),
        ("b.md", "---\ntags:\n  - nota\n  - tokio\n  - x\n---\n"),
        ("c.md", "---\ntags: tokio\n---\n"),
    ]);
    let plan = Fixture {
        files: fixture.files.clone(),
    }
    .tags()
    .plan_merge("#tokio", "#Nota")
    .unwrap();
    assert_eq!(
        plan.notices,
        [
            PlanNotice::Merges { into: key("nota") },
            PlanNotice::BothHavePages {
                from: path("tags/tokio.md"),
                into: path("tags/nota.md"),
            }
        ]
    );
    let mut without_page = fixture.files.clone();
    without_page.remove(&path("tags/tokio.md"));
    let fixture = Fixture {
        files: without_page,
    };
    let plan = fixture.tags().plan_merge("#tokio", "#Nota").unwrap();
    assert_eq!(plan.notices, [PlanNotice::Merges { into: key("nota") }]);
    assert_eq!(
        changed(&fixture, &plan),
        owned(&[
            ("a.md", "---\ntags: [nota]\n---\n#nota #nota\n"),
            ("b.md", "---\ntags:\n  - nota\n  - x\n---\n"),
            ("c.md", "---\ntags: nota\n---\n"),
            ("tags/nota.md", "---\nparents: []\n---\n"),
        ])
    );
}

#[test]
fn a_page_moves_to_the_merged_tag_when_only_it_has_one() {
    let fixture = Fixture::new(&[("tags/viejo.md", "Descripción"), ("a.md", "#nuevo #viejo")]);
    let plan = fixture.tags().plan_merge("#viejo", "#nuevo").unwrap();
    assert_eq!(
        plan.changes.renames,
        [Rename {
            from: path("tags/viejo.md"),
            to: path("tags/nuevo.md"),
        }]
    );
}

#[test]
fn rename_an_alias() {
    let fixture = fixture();
    let plan = fixture.tags().plan_rename("#rs", "#rst").unwrap();
    assert_eq!(plan.notices, [PlanNotice::RenamesAlias { of: key("rust") }]);
    assert_eq!(plan.changes.renames, []);
    assert_eq!(
        changed(&fixture, &plan),
        owned(&[
            (
                "a.md",
                "---\ntitle: Algo\ntags: [Rust, nota]  # comentario\n---\nHoy #rust y #RUST, también #rst.\n"
            ),
            (
                "tags/rust.md",
                "---\ncolor: naranja\naliases: [rst]\nparents: [lenguajes]\n---\nNotas de #rust.\n"
            ),
        ])
    );
}

#[test]
fn an_alias_cannot_take_the_name_of_a_page() {
    let plan = fixture().tags().plan_rename("#rs", "#gpui").unwrap();
    assert!(plan.is_blocked());
    assert_eq!(
        plan.notices[1],
        PlanNotice::AliasTaken {
            alias: key("gpui"),
            by: key("gpui")
        }
    );
}

#[test]
fn an_alias_can_absorb_a_tag_used_in_notes() {
    let plan = fixture().tags().plan_rename("#rs", "#nota").unwrap();
    assert_eq!(
        plan.notices,
        [
            PlanNotice::RenamesAlias { of: key("rust") },
            PlanNotice::AliasAbsorbs {
                tag: key("nota"),
                of: key("rust")
            },
        ]
    );
}

#[test]
fn use_the_main_name_of_an_alias() {
    let fixture = fixture();
    let plan = fixture.tags().plan_use_main_name("#rs").unwrap();
    assert_eq!(
        changed(&fixture, &plan),
        owned(&[(
            "a.md",
            "---\ntitle: Algo\ntags: [Rust, nota]  # comentario\n---\nHoy #rust y #RUST, también #rust.\n"
        )])
    );
}

#[test]
fn invalid_requests() {
    let tags = fixture().tags();
    assert_eq!(
        tags.plan_rename("#rust", "#a/b"),
        Err(PlanError::InvalidName("#a/b".into()))
    );
    assert_eq!(
        tags.plan_rename("#nada", "#x"),
        Err(PlanError::UnknownTag("#nada".into()))
    );
    assert_eq!(
        tags.plan_merge("#rust", "#nada"),
        Err(PlanError::UnknownTag("#nada".into()))
    );
    assert_eq!(
        tags.plan_use_main_name("#nota"),
        Err(PlanError::NotAnAlias("#nota".into()))
    );
}
