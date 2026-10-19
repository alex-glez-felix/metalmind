use pretty_assertions::assert_eq;

use super::super::test_session::Session;
use super::*;

fn marks(placed: &[(char, usize)]) -> GutterMarks {
    GutterMarks::new(placed.iter().map(|&(name, line)| PlacedMark { name, line }))
}

#[test]
fn cada_linea_muestra_la_primera_minusculas_antes() {
    let marks = marks(&[('B', 3), ('z', 3), ('a', 0), ('c', 3), ('A', 7)]);
    let shown: Vec<_> = (0..9)
        .filter_map(|line| {
            marks
                .at(line)
                .map(|label| (line, label.shown, label.more()))
        })
        .collect();
    assert_eq!(shown, [(0, 'a', false), (3, 'c', true), (7, 'A', false)]);
    assert_eq!(marks.at(3).map(|label| label.others), Some(&['z', 'B'][..]));
}

#[test]
fn el_tooltip_las_lista_todas() {
    let marks = marks(&[('B', 1), ('a', 1), ('q', 2)]);
    assert_eq!(
        marks.at(1).map(MarkLabel::describe).as_deref(),
        Some("marks a, B")
    );
    assert_eq!(
        marks.at(2).map(MarkLabel::describe).as_deref(),
        Some("mark q")
    );
    assert_eq!(marks.at(0), None);
}

#[test]
fn la_columna_no_depende_de_que_haya_marks() {
    assert_eq!(GutterMarks::columns(true), 2);
    assert_eq!(GutterMarks::columns(false), 3, "un espacio antes del texto");
}

/// Las marks de cada línea del panel enfocado: la que se ve y las demás.
fn shown(session: &Session) -> Vec<(usize, String)> {
    let marks = session
        .workspace
        .gutter_marks(session.pane())
        .unwrap()
        .unwrap();
    (0..=session
        .workspace
        .view(session.pane())
        .unwrap()
        .buffer()
        .last_line())
        .filter_map(|line| {
            let label = marks.at(line)?;
            Some((
                line,
                std::iter::once(label.shown)
                    .chain(label.others.iter().copied())
                    .collect(),
            ))
        })
        .collect()
}

#[test]
fn el_margen_sigue_a_las_marks_al_ponerlas_editar_y_borrarlas() {
    let mut session = Session::new("uno\ndos\ntres\ncuatro");
    assert_eq!(shown(&session), []);
    session.keys("majmbmBGmA");
    assert_eq!(
        shown(&session),
        [(0, "a".into()), (1, "bB".into()), (3, "A".into())]
    );
    session.keys("ggOx⎋");
    assert_eq!(
        shown(&session),
        [(1, "a".into()), (2, "bB".into()), (4, "A".into())]
    );
    session.keys(":delm b⏎");
    assert_eq!(
        shown(&session),
        [(1, "a".into()), (2, "B".into()), (4, "A".into())]
    );
    session.keys(":delm!⏎:delm A⏎");
    assert_eq!(shown(&session), [(2, "B".into())]);
    session.keys("3Gdd");
    assert_eq!(
        shown(&session),
        [(2, "B".into())],
        "una A–Z queda donde empezó lo borrado, como en vim"
    );
}

#[test]
fn las_de_archivo_solo_se_ven_en_su_archivo() {
    let notes = [("a.md", "uno\n[[b]]\n"), ("b.md", "b1\nb2\n")];
    let (_dir, _vault, mut session) = super::super::links::tests::vault_session(&notes, "a.md");
    session.keys("mAmajgd");
    assert_eq!(session.file(), "b.md");
    assert_eq!(shown(&session), []);
    session.keys("jmB⌃o");
    assert_eq!(session.file(), "a.md");
    assert_eq!(shown(&session), [(0, "aA".into())]);
}

#[test]
fn apagado_no_hay_marks_en_el_margen() {
    let mut session = Session::without_gutter_marks("uno");
    session.keys("ma");
    assert_eq!(
        session.workspace.gutter_marks(session.pane()).unwrap(),
        None
    );
}
