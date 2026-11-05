//! El plan de un renombre y su vista previa, sin GPUI: cómo se arma, qué hacer
//! con él cuando llega y cuándo una vista previa quedó atrás del índice.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use gpui_kit::Task;
use metalmind_vault::{Changes, LinkIndex, RenameConflict, RenamePlan, VaultPath};

use crate::change_preview::{ChangePreview, Notice};

/// Un renombre con su vista previa abierta: qué se renombra, el último
/// índice de links y lo que rehace la vista previa cuando llega otro.
pub(super) struct RenamePreview {
    from: VaultPath,
    to: VaultPath,
    index: Arc<LinkIndex>,
    /// La vista previa es de un índice anterior a `index`: aplicarla podría
    /// dejar fuera un link nuevo.
    behind: bool,
    rebuild: Option<Task<()>>,
}

impl RenamePreview {
    pub(super) fn new(from: VaultPath, to: VaultPath, index: Arc<LinkIndex>) -> Self {
        Self {
            from,
            to,
            index,
            behind: false,
            rebuild: None,
        }
    }

    /// Anota que llegó `latest`; `true` si la vista previa quedó atrás.
    pub(super) fn fall_behind(&mut self, latest: &Arc<LinkIndex>) -> bool {
        if Arc::ptr_eq(&self.index, latest) {
            return false;
        }
        self.index = Arc::clone(latest);
        self.behind = true;
        true
    }

    /// Anota que se puso la vista previa armada con `built_for`; `false`
    /// si ya llegó otro índice y no hay que ponerla.
    pub(super) fn catch_up(&mut self, built_for: &Arc<LinkIndex>) -> bool {
        let current = Arc::ptr_eq(&self.index, built_for);
        self.behind &= !current;
        current
    }

    pub(super) fn is_current(&self) -> bool {
        !self.behind
    }

    pub(super) fn paths(&self) -> (&VaultPath, &VaultPath) {
        (&self.from, &self.to)
    }

    pub(super) fn set_rebuild(&mut self, rebuild: Task<()>) {
        self.rebuild = Some(rebuild);
    }
}

/// El plan de un renombre y sus ediciones en líneas enteras.
pub(super) struct RenameBuild {
    pub(super) plan: RenamePlan,
    pub(super) changes: Changes,
}

/// Qué hacer con un plan que se terminó de armar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Arrival {
    Show,
    /// Llegó otro índice mientras se armaba: un link nuevo no estaría en el
    /// plan, así que se arma otra vez con el de ahora.
    Rebuild,
    /// Se abrió otra ventana flotante mientras tanto: no se le pasa por
    /// encima.
    Busy,
}

pub(super) fn arrival(
    built_for: &Arc<LinkIndex>,
    current: &Arc<LinkIndex>,
    overlay_open: bool,
) -> Arrival {
    if overlay_open {
        Arrival::Busy
    } else if Arc::ptr_eq(built_for, current) {
        Arrival::Show
    } else {
        Arrival::Rebuild
    }
}

/// El plan para renombrar `from` a `to` con `index`, con sus ediciones en
/// líneas enteras: lee cada archivo que se edita, así que va fuera del hilo
/// de la interfaz.
pub(super) fn build(
    index: &LinkIndex,
    root: &Path,
    from: &VaultPath,
    to: &VaultPath,
) -> Result<RenameBuild, String> {
    let plan = index
        .rename_plan(from, to)
        .map_err(|error| error.to_string())?;
    let changes = on_whole_lines(plan.changes(), root)?;
    Ok(RenameBuild { plan, changes })
}

/// Cada edición de `changes` en sus líneas enteras, para ver el link en su
/// línea antes y después: lee cada archivo que se edita.
fn on_whole_lines(mut changes: Changes, root: &Path) -> Result<Changes, String> {
    for file in &mut changes.edits {
        let text = fs::read_to_string(root.join(file.path.as_str()))
            .map_err(|error| format!("no se pudo leer {}: {error}", file.path))?;
        *file = file.whole_lines(&text).map_err(|error| error.to_string())?;
    }
    Ok(changes)
}

/// La vista previa de `changes`, los de `plan`: los links que cambian, y el
/// renombre, fijo.
pub(super) fn preview(
    plan: &RenamePlan,
    changes: Changes,
    unsaved: impl Fn(&VaultPath) -> bool,
) -> Option<ChangePreview> {
    let notices = conflicts_notice(&plan.conflicts, &plan.to)
        .into_iter()
        .collect();
    ChangePreview::new(title(&plan.from, &plan.to), changes, notices, unsaved)
        .map(ChangePreview::with_fixed_renames)
}

/// Cómo se llama el renombre en la vista previa y en los avisos.
pub(super) fn title(from: &VaultPath, to: &VaultPath) -> String {
    format!("Renombrar {from} a {to}")
}

/// Cuántos links quedan rotos porque no se pueden reescribir, y dónde
/// están los primeros: un solo aviso, que no impide aplicar, por muchos
/// que sean.
fn conflicts_notice(conflicts: &[RenameConflict], to: &VaultPath) -> Option<Notice> {
    let (first, rest) = conflicts.split_first()?;
    let at = |conflict: &RenameConflict| format!("{}:{}", conflict.source, conflict.line + 1);
    let mut places: Vec<String> = std::iter::once(first)
        .chain(rest.iter().take(SHOWN_CONFLICTS - 1))
        .map(at)
        .collect();
    if conflicts.len() > SHOWN_CONFLICTS {
        places.push(format!("y {} más", conflicts.len() - SHOWN_CONFLICTS));
    }
    let links = if conflicts.len() == 1 {
        "un wikilink no puede".to_owned()
    } else {
        format!("{} wikilinks no pueden", conflicts.len())
    };
    Some(Notice {
        text: format!(
            "{links} nombrar {to} (lleva # o |), quedarán rotos: {}",
            places.join(", ")
        ),
        blocks: false,
    })
}

/// Cuántos lugares de links rotos nombra el aviso, como mucho.
const SHOWN_CONFLICTS: usize = 3;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use metalmind_vault::{EntryKind, LinkIndex};
    use pretty_assertions::assert_eq;

    use super::*;

    fn path(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    /// La vista previa del renombre de `from` a `to` entre `notes`, con las
    /// ediciones en líneas enteras como las arma la ventana.
    fn preview_of(notes: &[(&str, &str)], from: &str, to: &str) -> ChangePreview {
        let plan = index(notes).rename_plan(&path(from), &path(to)).unwrap();
        let mut changes = plan.changes();
        for file in &mut changes.edits {
            let text = notes
                .iter()
                .find(|(name, _)| *name == file.path.as_str())
                .map(|(_, text)| *text)
                .unwrap();
            *file = file.whole_lines(text).unwrap();
        }
        preview(&plan, changes, |_| false).unwrap()
    }

    fn index(notes: &[(&str, &str)]) -> LinkIndex {
        let texts: HashMap<VaultPath, &str> = notes
            .iter()
            .map(|&(name, text)| (path(name), text))
            .collect();
        LinkIndex::build(
            texts.keys().map(|name| (name.clone(), EntryKind::Note)),
            |name| texts.get(name).map(|text| (*text).to_owned()),
        )
    }

    /// El renombre primero, elegido y fijo (quitarlo dejaría los links
    /// apuntando a la ruta vieja), y una fila por línea que cambia.
    #[test]
    fn los_links_que_cambian_y_el_renombre_fijo() {
        let notes = [
            ("a.md", "ver [[b]] y [[b#Meta]]\n"),
            ("b.md", "# B\n"),
            ("c.md", "[[b|la b]]\n"),
        ];
        let mut preview = preview_of(&notes, "b.md", "d.md");
        assert_eq!(preview.title(), "Renombrar b.md a d.md");
        assert_eq!(preview.selected(), 0, "el renombre, elegido");
        let rows: Vec<(&str, &str, &str)> = (0..preview.len())
            .map(|row| preview.row(row).unwrap())
            .map(|row| (row.path.as_str(), row.before, row.after))
            .collect();
        assert_eq!(
            rows,
            [
                ("b.md", "b.md", "d.md"),
                ("a.md", "ver [[b]] y [[b#Meta]]", "ver [[d]] y [[d#Meta]]"),
                ("c.md", "[[b|la b]]", "[[d|la b]]"),
            ]
        );
        assert!(preview.row(0).unwrap().fixed);
        assert!(!preview.row(1).unwrap().fixed);
        preview.toggle_selected();
        assert!(!preview.row(0).unwrap().dropped, "el renombre no se quita");
        preview.select(2);
        preview.toggle_selected();
        let changes = preview.changes();
        assert_eq!(changes.renames.len(), 1);
        assert_eq!(changes.edits.len(), 1, "solo a.md");
    }

    /// Un wikilink que no puede nombrar el destino se avisa sin impedir.
    #[test]
    fn un_link_que_no_se_puede_reescribir_se_avisa() {
        let notes = [("a.md", "uno\nver [[b]]\n"), ("b.md", "# B\n")];
        let preview = preview_of(&notes, "b.md", "b#2.md");
        assert_eq!(
            preview.notices(),
            [Notice {
                text: "un wikilink no puede nombrar b#2.md (lleva # o |), quedarán rotos: a.md:2"
                    .to_owned(),
                blocks: false,
            }]
        );
        assert_eq!(preview.blocked(), None);
    }

    /// Muchos links que no se pueden reescribir son un solo aviso, con los
    /// primeros lugares: si no, los avisos taparían la vista previa.
    #[test]
    fn muchos_links_rotos_son_un_solo_aviso() {
        let conflicts: Vec<RenameConflict> = (1..=5)
            .map(|at| RenameConflict {
                source: path(&format!("n{at}.md")),
                span: 0..5,
                line: at,
            })
            .collect();
        let notice = conflicts_notice(&conflicts, &path("b#2.md")).unwrap();
        assert_eq!(
            notice.text,
            "5 wikilinks no pueden nombrar b#2.md (lleva # o |), quedarán rotos: \
             n1.md:2, n2.md:3, n3.md:4, y 2 más"
        );
        assert!(!notice.blocks);
        assert_eq!(conflicts_notice(&[], &path("b.md")), None);
    }

    /// `build` lee los archivos del disco: arma las líneas con el texto de
    /// ahora, y si un archivo ya no dice lo que el índice cree, falla en vez
    /// de armar ediciones que después no se aplicarían.
    #[test]
    fn el_plan_se_arma_con_los_archivos_de_ahora() {
        let notes = [("a.md", "uno\nver [[b]]\n"), ("b.md", "# B\n")];
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in notes {
            std::fs::write(dir.path().join(name), text).unwrap();
        }
        let built = build(&index(&notes), dir.path(), &path("b.md"), &path("d.md")).unwrap();
        let lines: Vec<(&str, &str)> = built.changes.edits[0]
            .edits
            .iter()
            .map(|edit| (edit.old.as_str(), edit.new.as_str()))
            .collect();
        assert_eq!(lines, [("ver [[b]]", "ver [[d]]")]);
        std::fs::write(dir.path().join("a.md"), "otra cosa\n").unwrap();
        let error = build(&index(&notes), dir.path(), &path("b.md"), &path("d.md"));
        assert!(error.is_err());
    }

    /// Un plan armado se muestra solo si sigue siendo del índice de ahora y
    /// no se abrió otra ventana flotante mientras tanto.
    #[rstest::rstest]
    #[case::el_mismo_indice(true, false, Arrival::Show)]
    #[case::otro_indice(false, false, Arrival::Rebuild)]
    #[case::otra_ventana(true, true, Arrival::Busy)]
    #[case::otra_ventana_y_otro_indice(false, true, Arrival::Busy)]
    fn que_hacer_con_un_plan_armado(
        #[case] same_index: bool,
        #[case] overlay_open: bool,
        #[case] expected: Arrival,
    ) {
        let built_for = Arc::new(index(&[("a.md", "")]));
        let current = if same_index {
            Arc::clone(&built_for)
        } else {
            Arc::new(index(&[("a.md", "")]))
        };
        assert_eq!(arrival(&built_for, &current, overlay_open), expected);
    }

    /// Mientras llega otro índice, la vista previa queda atrás hasta que se
    /// pone la que se armó con el último; la de un índice intermedio no
    /// cuenta, y si no se pudo rehacer sigue atrás.
    #[test]
    fn la_vista_previa_queda_atras_hasta_ponerse_la_del_ultimo_indice() {
        let first = Arc::new(index(&[("a.md", "")]));
        let mut rename = RenamePreview::new(path("a.md"), path("b.md"), Arc::clone(&first));
        assert!(rename.is_current());
        assert!(
            !rename.fall_behind(&first),
            "el mismo índice no la deja atrás"
        );
        let second = Arc::new(index(&[("a.md", "")]));
        let third = Arc::new(index(&[("a.md", "")]));
        assert!(rename.fall_behind(&second));
        assert!(rename.fall_behind(&third));
        assert!(!rename.is_current());
        assert!(
            !rename.catch_up(&second),
            "la de un índice intermedio no se pone"
        );
        assert!(!rename.is_current());
        assert!(rename.catch_up(&third));
        assert!(rename.is_current());
    }
}
