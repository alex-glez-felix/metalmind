# metalmind-keymap

El registro de acciones de Metalmind, sin GUI: el nombre de cada acción y los atajos de todos los contextos.

- `Action`: todas las acciones con nombre. Las de edición son las del motor (`Action::Vim`, con los nombres de `metalmind_vim::Action`); las de la ventana son `enum`s de este crate, una variante por acción con su nombre al lado: `Command` (guardar, cerrar, paneles, finder, paleta, panel lateral, seguir un link), `EditorAction` (portapapeles), `PanelAction` (el árbol, sus preguntas y su menú) y `PickerAction`. `Action::from_name` y `Action::all` son la única lista; `Action::owner` dice qué vista la atiende.
- `Invocation`: una acción con lo escrito tras su nombre en `:` (`:e notas/a.md`). `Invocation::resolve(nombre, argumento)` es lo que hacen `:`, la paleta y los atajos.
- `Keymap::default()`: una sola lista de `Binding`s. Cada uno dice dónde vale y con qué teclas (`Keys::Vim(contexto de vim, teclas)` lo resuelve el motor; `Keys::View(View, teclas)`, la ventana) y qué acción invoca. Las teclas de edición vienen de `metalmind_vim::Keymap::vim()`; los atajos de la ventana están en `src/defaults.rs`, y los que valen en el editor y en el árbol (`space e`, `space t`) se declaran una vez.
- `Keymap::engine()` es la tabla del motor: sus acciones y, por nombre, las de la ventana que se teclean en el editor (`space f`, `ctrl-w v`, `gd`), más los comandos de `:` de `Command`. `Keymap::view_bindings()` son los atajos que registra la ventana en su framework, que solo los transporta. `Keymap::keys_for(acción, vistas activas)` da la tecla que se muestra junto a la acción en la paleta y en los menús.
- `Keystroke` es una tecla de un atajo de la ventana, con más teclas y modificadores que `metalmind_vim::Key` (`home`, `ctrl-shift-p`); se escribe igual que ella.
- Contextos de vista (`context::FILE_TREE`, `View::predicate`) con la sintaxis de Zed y GPUI: `Picker && splits`.

Tests: nombres únicos y que cada uno resuelve a su acción, tabla del motor sin secuencias ambiguas, atajos de vista sin duplicados, y que el motor solo devuelve nombres del registro. Las formas de `:` de `Command` se comparan con nvim en `metalmind-vim/tests/`.
