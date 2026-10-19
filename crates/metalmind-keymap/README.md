# metalmind-keymap

El registro de acciones de Metalmind, sin GUI: el nombre de cada acción y sus atajos.

- `Action`: todas las acciones con nombre. Las de edición son las del motor (`Action::Vim`, con los nombres de `metalmind_vim::Action`); los comandos de la ventana son `Command` (guardar, cerrar, paneles, finder, paleta, panel lateral, seguir un link), una variante por comando con su nombre al lado. `Action::from_name` y `Action::all` son la única lista.
- `Invocation`: una acción con lo escrito tras su nombre en `:` (`:e notas/a.md`). `Invocation::resolve(nombre, argumento)` es lo que hacen `:`, la paleta y los atajos.
- `Keymap::default()`: las teclas de edición de `metalmind_vim::Keymap::vim()` y los atajos de la ventana que se teclean en el editor (`space f`, `ctrl-w v`, `gd`, en `src/defaults.rs`). `Keymap::engine()` es la tabla del motor: sus acciones y, por nombre, las de la ventana, más los comandos de `:` de `Command`.

Tests: nombres únicos y que cada uno resuelve a su acción, tabla del motor sin secuencias ambiguas y que el motor solo devuelve nombres del registro. Las formas de `:` de `Command` se comparan con nvim en `metalmind-vim/tests/`.
