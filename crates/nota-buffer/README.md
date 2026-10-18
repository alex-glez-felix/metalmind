# nota-buffer

Texto editable de `nota`, sin GUI ni I/O.

- `Buffer`: rope (`ropey`) con líneas separadas solo por `\n`; `line_text` da una línea sin su `\n`, y `lines_text` varias seguidas sin el `\n` de la última.
- `ByteOffset`: todo offset es en bytes; columnas en caracteres solo con `column_of` / `offset_at_column`.
- `Selection` / `Selections`: siempre al menos una, ordenadas y sin solapes (multi-cursor).
- `Edit` + `Buffer::apply`: lote de ediciones en coordenadas previas; `EditMap` traduce posiciones al texto nuevo.
- Transacciones: `begin_transaction` … `commit_transaction` forman un paso de undo/redo con las selecciones de antes y después.
- Historial en árbol: deshacer y editar abre una rama sin perder la anterior. `undo` / `redo` recorren la rama actual; `earlier` / `later` recorren todos los estados por `HistorySeq`, en orden cronológico.
- Grupos de undo: entre `begin_undo_group` y `end_undo_group` las transacciones se suman en un solo paso, como los cambios de una macro de vim. Moverse por el historial corta el grupo. Un grupo abierto dentro de otro (un `:g` en una macro) es parte del de afuera.
- Líneas marcadas (`mark_lines`, `pop_marked_line`), como las de `:g` de vim (`ml_setmarked`): siguen a su línea con cada edición, también de undo, y se pierden si se borra. Las marcas no cambian de orden, así que el corrimiento de las que quedan después de una edición va en un árbol de Fenwick: cada edición cuesta O(log² n) más las marcas de sus propias líneas.
- `LineAnchor`: posición que sigue a su línea como una mark de vim (`mark_adjust`). Se corre cuando se insertan o borran líneas antes, conserva su columna en bytes con ediciones en su línea, se mueve con su texto si su línea se une a la anterior y se pierde si se borra su línea. Cada transacción guarda dónde estaban las anclas antes del cambio, y undo y redo las devuelven ahí.
- `Splice` + `Buffer::take_splices`: cada reemplazo hecho en el texto, también los de undo y redo, para mover posiciones guardadas fuera del motor (el cursor de otro panel sobre el mismo buffer). Quien los use los toma tras cada cambio; si nadie los toma, la lista crece.

No sabe nada de vim, archivos ni markdown.
