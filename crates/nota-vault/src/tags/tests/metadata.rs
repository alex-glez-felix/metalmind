//! Poner color, ícono, alias y padres desde una acción (05 §3, §9): crea la
//! página si no hay y, si hay, cambia solo ese campo.

use pretty_assertions::assert_eq;

use super::{Fixture, applied, changed, key, owned, path};
use crate::tags::{PageChange, PlanError, PlanNotice, TagColor, TagIcon, Tags};

fn fixture() -> Fixture {
    Fixture::new(&[
        (
            "tags/rust.md",
            "---\ntitle: Rust  # a mano\ncolor: rojo\naliases: [rs]\nparents:\n  - lenguajes\n\n# fin\n---\nCuerpo.\n",
        ),
        ("tags/sin-frontmatter.md", "De qué trata.\n"),
        ("tags/bom.md", "\u{FEFF}Con BOM.\n"),
        ("tags/crlf.md", "---\r\ncolor: azul\r\n---\r\n"),
        ("tags/roto.md", "---\ncolor: [rojo\n---\n"),
        ("tags/flujo.md", "---\n{color: rojo}\n---\n"),
        (
            "tags/doble.md",
            "---\ncolor: rojo\nicon: 🦀\ncolor: azul\n---\n",
        ),
        ("a.md", "#Gpui y #gpui y #GPUI, #nota, #con, #rs"),
        ("b.md", "#Gpui"),
    ])
}

/// Lo que cambia `change` sobre `tag` en el vault de [`fixture`].
fn change(tag: &str, change: &PageChange) -> Vec<(String, String)> {
    let fixture = fixture();
    let plan = fixture.tags().plan_page_change(tag, change).unwrap();
    assert!(!plan.is_blocked(), "{:?}", plan.notices);
    changed(&fixture, &plan)
}

/// Los tags tras aplicar `change`, para ver que el campo se lee de vuelta.
fn after(tag: &str, change: &PageChange) -> Tags {
    let fixture = fixture();
    let plan = fixture.tags().plan_page_change(tag, change).unwrap();
    Fixture {
        files: applied(&fixture, &plan),
    }
    .tags()
}

fn named(color: &str) -> PageChange {
    PageChange::Color(Some(TagColor::Named(color.into())))
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_tag_without_page_gets_one_with_only_that_field() {
    assert_eq!(
        change("#gpui", &named("naranja")),
        owned(&[("tags/Gpui.md", "---\ncolor: naranja\n---\n")])
    );
    let tags = after("gpui", &named("naranja"));
    let page = tags.graph().page(&key("gpui")).unwrap();
    assert_eq!(page.color, Some(TagColor::Named("naranja".into())));
    assert_eq!(tags.display_name(&key("gpui")), Some("Gpui"));
}

#[test]
fn a_hex_color_and_an_icon_are_quoted_when_yaml_needs_it() {
    let hex = PageChange::Color(Some(TagColor::Hex("#d97706".into())));
    assert_eq!(
        change("nota", &hex),
        owned(&[("tags/nota.md", "---\ncolor: \"#d97706\"\n---\n")])
    );
    let keycap = PageChange::Icon(Some(TagIcon::Emoji("#️⃣".into())));
    assert_eq!(
        change("nota", &keycap),
        owned(&[("tags/nota.md", "---\nicon: \"#️⃣\"\n---\n")])
    );
    let page = |tags: Tags| tags.graph().page(&key("nota")).cloned().unwrap();
    assert_eq!(
        page(after("nota", &hex)).color,
        Some(TagColor::Hex("#d97706".into()))
    );
    assert_eq!(
        page(after("nota", &keycap)).icon,
        Some(TagIcon::Emoji("#️⃣".into()))
    );
}

#[test]
fn a_hand_written_page_changes_only_that_field() {
    let rust = "---\ntitle: Rust  # a mano\ncolor: naranja\naliases: [rs]\nparents:\n  - lenguajes\n\n# fin\n---\nCuerpo.\n";
    assert_eq!(
        change("rust", &named("naranja")),
        owned(&[("tags/rust.md", rust)])
    );
    let set = PageChange::Icon(Some(TagIcon::Set {
        set: "tabler".into(),
        name: "brand-rust".into(),
    }));
    let with_icon = "---\ntitle: Rust  # a mano\ncolor: rojo\naliases: [rs]\nparents:\n  - lenguajes\n\n# fin\nicon: tabler:brand-rust\n---\nCuerpo.\n";
    assert_eq!(change("rust", &set), owned(&[("tags/rust.md", with_icon)]));
    let parents = "---\ntitle: Rust  # a mano\ncolor: rojo\naliases: [rs]\nparents: [lenguajes, \"2024\"]\n\n# fin\n---\nCuerpo.\n";
    assert_eq!(
        change(
            "rust",
            &PageChange::Parents(names(&["lenguajes", "#2024", "Lenguajes"]))
        ),
        owned(&[("tags/rust.md", parents)])
    );
    let no_aliases =
        "---\ntitle: Rust  # a mano\ncolor: rojo\nparents:\n  - lenguajes\n\n# fin\n---\nCuerpo.\n";
    assert_eq!(
        change("rust", &PageChange::Aliases(Vec::new())),
        owned(&[("tags/rust.md", no_aliases)])
    );
}

#[test]
fn a_template_is_set_through_an_alias_and_read_back() {
    let template = PageChange::Template(Some("trabajo/reunión: semanal".into()));
    let rust = "---\ntitle: Rust  # a mano\ncolor: rojo\naliases: [rs]\nparents:\n  - lenguajes\n\n# fin\ntemplate: \"trabajo/reunión: semanal\"\n---\nCuerpo.\n";
    assert_eq!(change("#rs", &template), owned(&[("tags/rust.md", rust)]));
    let tags = after("rust", &template);
    let page = tags.graph().page(&key("rust")).unwrap();
    assert_eq!(page.template.as_deref(), Some("trabajo/reunión: semanal"));
    assert_eq!(
        change("gpui", &PageChange::Template(Some("diario".into()))),
        owned(&[("tags/Gpui.md", "---\ntemplate: diario\n---\n")])
    );
}

#[test]
fn an_alias_changes_the_page_of_its_tag() {
    let rust = "---\ntitle: Rust  # a mano\ncolor: verde\naliases: [rs]\nparents:\n  - lenguajes\n\n# fin\n---\nCuerpo.\n";
    assert_eq!(
        change("#rs", &named("verde")),
        owned(&[("tags/rust.md", rust)])
    );
}

#[test]
fn a_page_without_frontmatter_gets_one_before_its_body() {
    assert_eq!(
        change("sin-frontmatter", &named("azul")),
        owned(&[(
            "tags/sin-frontmatter.md",
            "---\ncolor: azul\n---\nDe qué trata.\n"
        )])
    );
    assert_eq!(
        change("bom", &named("azul")),
        owned(&[("tags/bom.md", "\u{FEFF}---\ncolor: azul\n---\nCon BOM.\n")])
    );
    assert_eq!(
        after("bom", &named("azul"))
            .graph()
            .page(&key("bom"))
            .unwrap()
            .color,
        Some(TagColor::Named("azul".into()))
    );
}

#[test]
fn crlf_pages_keep_their_line_endings() {
    let icon = PageChange::Icon(Some(TagIcon::Emoji("🦀".into())));
    assert_eq!(
        change("crlf", &icon),
        owned(&[("tags/crlf.md", "---\r\ncolor: azul\r\nicon: 🦀\r\n---\r\n")])
    );
}

#[test]
fn nothing_to_remove_is_an_empty_plan() {
    let fixture = fixture();
    let tags = fixture.tags();
    for (tag, change) in [
        ("gpui", PageChange::Color(None)),
        ("rust", PageChange::Icon(None)),
        ("sin-frontmatter", PageChange::Parents(Vec::new())),
    ] {
        let plan = tags.plan_page_change(tag, &change).unwrap();
        assert!(plan.changes.is_empty(), "{tag}: {plan:?}");
    }
}

#[test]
fn creating_an_empty_page_for_its_description() {
    let fixture = fixture();
    let tags = fixture.tags();
    let plan = tags.plan_create_page("#GPUI").unwrap();
    assert_eq!(changed(&fixture, &plan), owned(&[("tags/Gpui.md", "")]));
    assert!(tags.plan_create_page("rs").unwrap().changes.is_empty());
    assert_eq!(tags.page_path("rs").unwrap(), path("tags/rust.md"));
    assert_eq!(tags.page_path("gpui").unwrap(), path("tags/Gpui.md"));
}

#[test]
fn notices_for_aliases_and_reserved_names() {
    let tags = fixture().tags();
    let plan = tags
        .plan_page_change("rust", &PageChange::Aliases(names(&["rs", "crlf", "nota"])))
        .unwrap();
    assert_eq!(
        plan.notices,
        [
            PlanNotice::AliasTaken {
                alias: key("crlf"),
                by: key("crlf")
            },
            PlanNotice::AliasAbsorbs {
                tag: key("nota"),
                of: key("rust")
            },
        ]
    );
    assert!(plan.is_blocked());
    let plan = tags.plan_page_change("con", &named("rojo")).unwrap();
    assert_eq!(
        plan.notices,
        [PlanNotice::NotPortable {
            path: path("tags/con.md")
        }]
    );
    assert!(!plan.is_blocked());
}

#[test]
fn invalid_requests() {
    let tags = fixture().tags();
    let error = |tag: &str, change: PageChange| tags.plan_page_change(tag, &change).unwrap_err();
    assert_eq!(
        error("a/b", named("rojo")),
        PlanError::InvalidName("a/b".into())
    );
    assert_eq!(
        error("rust", named("rojo oscuro")),
        PlanError::InvalidValue("rojo oscuro".into())
    );
    assert_eq!(
        error(
            "rust",
            PageChange::Icon(Some(TagIcon::Emoji("libro".into())))
        ),
        PlanError::InvalidValue("libro".into())
    );
    assert_eq!(
        error("rust", PageChange::Template(Some("../fuera".into()))),
        PlanError::InvalidValue("../fuera".into())
    );
    assert_eq!(
        error("rust", PageChange::Parents(names(&["ui", "rs"]))),
        PlanError::OwnParent("rs".into())
    );
    assert_eq!(
        error("rust", PageChange::Aliases(names(&["x y"]))),
        PlanError::InvalidName("x y".into())
    );
    for page in ["roto", "flujo", "doble"] {
        assert_eq!(
            error(page, named("rojo")),
            PlanError::UneditablePage(path(&format!("tags/{page}.md")))
        );
    }
}
