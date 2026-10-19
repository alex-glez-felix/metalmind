//! Cómo se marca una fila del árbol. El cursor del árbol y la nota abierta
//! en el panel de edición son dos cosas, como el "archivo activo" de Zed, y
//! se ven distintas; el cursor sin foco se ve atenuado, para que no parezca
//! que `enter` va a llegar al árbol.

/// El fondo de una fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// El cursor del árbol, con el foco en el árbol: el acento del tema.
    Selection,
    /// La nota del panel de edición enfocado.
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowMarks {
    pub fill: Option<Fill>,
    /// El cursor del árbol con el foco en otro lado: solo un borde tenue.
    pub outline: bool,
}

impl RowMarks {
    #[must_use]
    pub fn of(selected: bool, tree_focused: bool, active: bool) -> Self {
        let fill = if selected && tree_focused {
            Some(Fill::Selection)
        } else if active {
            Some(Fill::Active)
        } else {
            None
        };
        Self {
            fill,
            outline: selected && !tree_focused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::nada(false, false, false, None, false)]
    #[case::cursor_con_foco(true, true, false, Some(Fill::Selection), false)]
    #[case::cursor_sin_foco(true, false, false, None, true)]
    #[case::activa(false, false, true, Some(Fill::Active), false)]
    #[case::activa_con_el_foco_en_el_arbol(false, true, true, Some(Fill::Active), false)]
    #[case::cursor_sobre_la_activa_con_foco(true, true, true, Some(Fill::Selection), false)]
    #[case::cursor_sobre_la_activa_sin_foco(true, false, true, Some(Fill::Active), true)]
    fn marca_el_cursor_y_la_nota_activa_por_separado(
        #[case] selected: bool,
        #[case] tree_focused: bool,
        #[case] active: bool,
        #[case] fill: Option<Fill>,
        #[case] outline: bool,
    ) {
        assert_eq!(
            RowMarks::of(selected, tree_focused, active),
            RowMarks { fill, outline }
        );
    }
}
