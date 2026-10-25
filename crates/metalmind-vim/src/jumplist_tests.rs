use metalmind_buffer::{Buffer, Edit, Point, Selection, Selections};
use proptest::prelude::*;

use super::*;

fn at(file: u32, line: usize) -> Jump {
    Jump {
        file: FileId::new(file),
        point: Point::new(line, 0),
    }
}

fn lines(list: &JumpList) -> Vec<(u32, usize)> {
    list.entries()
        .iter()
        .map(|jump| (jump.file.0, jump.point.line))
        .collect()
}

fn all(_: FileId) -> bool {
    true
}

fn list_of(jumps: &[(u32, usize)]) -> JumpList {
    let mut list = JumpList::default();
    for &(file, line) in jumps {
        list.push(at(file, line));
    }
    list
}

#[test]
fn guarda_hasta_cien_y_pierde_la_mas_vieja() {
    let mut list = JumpList::default();
    for line in 0..=CAPACITY {
        list.push(at(0, line));
    }
    assert_eq!(list.entries().len(), CAPACITY);
    assert_eq!(list.entries()[0], at(0, 1));
    assert_eq!(list.index(), CAPACITY);
}

#[test]
fn de_la_misma_linea_queda_la_ultima() {
    let mut list = list_of(&[(0, 1), (0, 5), (1, 1), (0, 1), (0, 5)]);
    list.cleanup(at(0, 9));
    assert_eq!(lines(&list), [(1, 1), (0, 1), (0, 5)]);
    assert_eq!(list.index(), 3);
}

#[test]
fn al_final_se_quita_la_entrada_de_la_linea_actual() {
    let mut list = list_of(&[(0, 1), (0, 5)]);
    list.cleanup(at(0, 5));
    assert_eq!(lines(&list), [(0, 1)]);
    assert_eq!(list.index(), 1);
}

#[test]
fn el_primer_ctrl_o_agrega_donde_esta_el_cursor() {
    let mut list = list_of(&[(0, 1), (0, 5)]);
    let step = list.step(Direction::Older, 1, at(0, 9), all).unwrap();
    assert_eq!(step.target, at(0, 5));
    assert!(step.pushed_here);
    assert_eq!(lines(&list), [(0, 1), (0, 5), (0, 9)]);
    assert_eq!(list.index(), 1);
    let step = list.step(Direction::Older, 1, at(0, 5), all).unwrap();
    assert_eq!((step.target, step.pushed_here), (at(0, 1), false));
    assert_eq!(list.step(Direction::Older, 1, at(0, 1), all), None);
    let step = list.step(Direction::Newer, 2, at(0, 1), all).unwrap();
    assert_eq!(step.target, at(0, 9));
    assert_eq!(list.step(Direction::Newer, 1, at(0, 9), all), None);
}

#[test]
fn ctrl_i_al_final_no_hace_nada() {
    let mut list = list_of(&[(0, 1)]);
    assert_eq!(list.step(Direction::Newer, 1, at(0, 9), all), None);
    assert_eq!(lines(&list), [(0, 1)]);
    assert_eq!(
        JumpList::default().step(Direction::Older, 1, at(0, 9), all),
        None
    );
}

#[test]
fn salta_las_entradas_de_archivos_que_ya_no_existen() {
    let mut list = list_of(&[(0, 1), (1, 3), (2, 4)]);
    let exists = |file: FileId| file.0 != 1 && file.0 != 2;
    let step = list.step(Direction::Older, 1, at(0, 9), exists).unwrap();
    assert_eq!(step.target, at(0, 1));
    assert_eq!(list.index(), 0);
}

#[test]
fn las_entradas_siguen_a_su_linea() {
    let mut buffer = Buffer::from("a\nb\nc\nd");
    let mut list = list_of(&[(0, 2), (1, 2), (0, 3)]);
    let selections = Selections::single(Selection::cursor(metalmind_buffer::ByteOffset::ZERO));
    buffer.begin_transaction(selections.clone()).unwrap();
    buffer
        .apply(vec![Edit::delete(
            metalmind_buffer::ByteOffset::new(2)..metalmind_buffer::ByteOffset::new(6),
        )])
        .unwrap();
    buffer.commit_transaction(selections).unwrap();
    list.adjust(FileId::new(0), &buffer.take_splices());
    assert_eq!(lines(&list), [(0, 1), (1, 2), (0, 1)]);
}

/// Lo que se guarda sigue lo editado en el archivo del panel que la lista
/// todavía no siguió; lo de los otros archivos ya lo siguió quien editó.
#[test]
fn lo_que_se_guarda_sigue_lo_editado_en_el_archivo_del_panel() {
    let mut buffer = Buffer::from("a\nb\nc\nd");
    let doc = DocState::default();
    let list = list_of(&[(0, 2), (1, 2), (0, 3)]);
    let selections = Selections::single(Selection::cursor(metalmind_buffer::ByteOffset::ZERO));
    buffer.begin_transaction(selections.clone()).unwrap();
    buffer
        .apply(vec![Edit::insert(
            metalmind_buffer::ByteOffset::ZERO,
            "x\n",
        )])
        .unwrap();
    buffer.commit_transaction(selections).unwrap();
    let saved = list.followed(FileId::new(0), &doc, &buffer);
    assert_eq!(saved, [at(0, 3), at(1, 2), at(0, 4)]);
    assert_eq!(lines(&list), [(0, 2), (1, 2), (0, 3)], "la lista no cambia");
}

#[test]
fn una_lista_recuperada_queda_sin_retroceder_y_con_las_ultimas_cien() {
    let list = JumpList::restored((0..CAPACITY + 5).map(|line| at(0, line)));
    assert_eq!(list.entries().len(), CAPACITY);
    assert_eq!(list.entries()[0], at(0, 5));
    assert_eq!(list.index(), CAPACITY);
    assert_eq!(JumpList::restored([]), JumpList::default());
}

#[derive(Clone, Debug)]
enum Op {
    Push(u32, usize),
    Step(Direction, usize, u32, usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0..3u32, 0..8usize).prop_map(|(f, l)| Op::Push(f, l)),
        2 => (any::<bool>(), 0..4usize, 0..3u32, 0..8usize).prop_map(|(older, c, f, l)| {
            let direction = if older { Direction::Older } else { Direction::Newer };
            Op::Step(direction, c, f, l)
        }),
    ]
}

proptest! {
    /// Haga lo que se haga, el índice no pasa del largo, no hay más de
    /// cien, lo que devuelve es la entrada del índice y nunca de un archivo
    /// que no existe; y `ctrl-o` seguido de `ctrl-i` vuelve a la línea de
    /// antes.
    #[test]
    fn la_lista_queda_coherente(ops in prop::collection::vec(op(), 0..300), gone in 0..4u32) {
        let mut list = JumpList::default();
        let exists = |file: FileId| file.0 != gone;
        for op in ops {
            match op {
                Op::Push(file, line) => list.push(at(file, line)),
                Op::Step(direction, count, file, line) => {
                    let here = at(file, line);
                    if let Some(step) = list.step(direction, count, here, exists) {
                        prop_assert_eq!(list.entries()[list.index()], step.target);
                        prop_assert!(step.target.file == here.file || exists(step.target.file));
                    }
                }
            }
            prop_assert!(list.entries().len() <= CAPACITY);
            prop_assert!(list.index() <= list.entries().len());
        }
    }

    #[test]
    fn ctrl_o_y_ctrl_i_vuelven_a_la_misma_linea(
        jumps in prop::collection::vec((0..3u32, 0..8usize), 0..150),
        here in (0..3u32, 0..8usize),
    ) {
        let mut list = list_of(&jumps);
        let here = at(here.0, here.1);
        if let Some(back) = list.step(Direction::Older, 1, here, all) {
            let forward = list.step(Direction::Newer, 1, back.target, all).unwrap();
            prop_assert_eq!(forward.target, here);
        }
    }
}
