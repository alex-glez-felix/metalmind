//! Lo que le cuenta el motor a quien lo aloja tras cada tecla.

use crate::{
    Action, CommandError, GlobalMiss, Invocation, Landing, SearchError, Substitution, Wrap,
};

/// Qué pasó con una tecla o una acción.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Falta el resto de la secuencia o del count.
    Pending,
    Executed(Action),
    /// Texto escrito en modo Insert o en la línea de comandos.
    Inserted,
    /// Ningún binding la usa; la capa de arriba puede probar otro keymap.
    Unbound,
    /// Se envió la línea de comandos, o se tecleó un atajo como `ctrl-w v`,
    /// con una acción que ejecuta quien aloja el editor.
    Dispatch(Invocation),
    /// Un salto a otro archivo: quien aloja el editor cambia el buffer del
    /// panel y llama a [`Vim::land`](crate::Vim::land). Una macro se pausa
    /// aquí, como con un comando.
    SwitchFile(Landing),
    /// Se envió la línea de comandos con algo que no es un comando, o con
    /// un `:s` que falló.
    InvalidCommand(CommandError),
    /// Una búsqueda llegó a un borde y siguió por el otro; la UI lo avisa.
    SearchWrapped(Wrap),
    /// Una búsqueda no movió el cursor: no encontró nada o no se pudo hacer.
    SearchFailed(SearchError),
    /// Un `:s`, o los de un `:g`, reemplazaron algo.
    Substituted(Substitution),
    /// Un `:g` no encontró líneas en las que correr.
    GlobalMissed(GlobalMiss),
}
