# metalmind-ui

Las vistas de Metalmind sobre GPUI. Es el único crate, junto al binario, que depende de la GUI; lo que no pinta vive en módulos sin GPUI, con sus tests.

Sin GPUI:

- `workspace/`: lo que hay abierto en una ventana. Los documentos, el árbol de splits con el cursor de cada panel y un solo motor vim para todos; los comandos de `:`, `ctrl-w` y `space`, y lo que el panel lateral le pide a los paneles (`open_file`, `rename_file`, `has_unsaved_within`).
- `document.rs`: una nota abierta, o un buffer `[No Name]`; el `fixeol` y el guardado.
- `file_tree/`: el árbol de carpetas del panel lateral sobre el `VaultIndex`: filas visibles, carpetas abiertas, selección, filtro difuso y los cambios del watcher. Benchmark en `benches/file_tree.rs`.
- `panel/`: el panel lateral entero (si se ve, en qué vista) y lo que le pregunta al usuario: un nombre, el filtro o confirmar un borrado. Confirmar da un `Request` con la operación de archivo.
- `session.rs`: lo que se recuerda entre sesiones en el directorio de estado: el último vault y, por vault, el panel.
- `picker/model.rs`: el estado del picker difuso (`PickerState`) del finder y la paleta. Benchmark en `benches/picker.rs`.
- `text_search/`: `space /`. El estado (`TextSearchState`: lo escrito, los resultados y sus versiones), el hilo que cancela y lanza cada búsqueda (`SearchWorker`) y el contexto de cada fila. Benchmark en `benches/text_search.rs`.

Con GPUI:

- `window.rs`: abre la ventana con un `Startup` (el vault, su watcher y el primer documento).
- `workspace_view.rs`: la raíz. La barra de botones, el panel lateral y un `Editor` por panel; mueve el foco entre ellos.
- `editor/`: un panel de edición. Pinta solo lo visible y le pasa las teclas al motor vim.
- `picker/`: la vista de un picker (`PickerView`), una sola para el finder, la paleta y `space /`; lo que cambia lo pone su `PickerDelegate`.
- `sidebar/`: el panel lateral. El árbol en una `uniform_list` (solo se pintan las filas visibles), el menú del clic derecho, los botones y las acciones con nombre (`file_tree::*`, `panel::*`) con sus atajos.
- `theme.rs`: todo color y tamaño de la UI sale de aquí.

## El panel lateral

| Tecla | Acción | Qué hace |
|---|---|---|
| `space e` / `space t` (en un editor) | `panel::toggle_folders` / `panel::toggle_tags` | Muestra el panel en esa vista con el foco, o lo esconde si ya la muestra. También con los botones de la barra de la izquierda. |
| `j` `k` `gg` `G` | `file_tree::select_next` … | Moverse por las filas. |
| `h` / `l` | `file_tree::collapse_or_parent` / `file_tree::expand_or_open` | Cerrar la carpeta o subir; abrir la carpeta, bajar a su contenido o abrir el archivo. |
| `o` `Enter`, doble clic | `file_tree::open` | Abrir en el panel de edición enfocado (o abrir y cerrar una carpeta). |
| `ctrl-v` / `ctrl-s` | `file_tree::open_split_right` / `file_tree::open_split_down` | Abrir en un split. |
| `/` | `file_tree::filter` | Filtrar con búsqueda difusa; `Enter` lo deja puesto y `Escape` lo quita. |
| `a` / `A` | `file_tree::new_note` / `file_tree::new_folder` | Crear dentro de la carpeta seleccionada (o junto al archivo). Un nombre que termina en `/` es una carpeta; a una nota sin `.md` se le agrega. |
| `r` / `m` | `file_tree::rename` / `file_tree::move` | Renombrar (el nombre) o mover (la ruta entera). Los buffers abiertos siguen al archivo. |
| `d` | `file_tree::delete` | Mandar a la papelera del vault (`.trash/`), tras confirmar con `y` o `Enter`. |
| `T` / `q` | `panel::toggle_view` / `panel::hide` | Cambiar de vista; esconder el panel. |
| `ctrl-w l` | `panel::focus_editor` | Volver al editor; desde el editor, `ctrl-w h` sin panel a la izquierda vuelve al árbol. |

Con el ratón, un clic selecciona (y abre o cierra una carpeta), el doble clic abre y el clic derecho abre un menú con las mismas acciones.
