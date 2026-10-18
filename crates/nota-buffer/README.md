# nota-buffer

Texto editable de `nota`, sin GUI ni I/O.

- `Buffer`: rope (`ropey`) con líneas separadas solo por `\n`; `line_text` da una línea sin su `\n`.
- `ByteOffset`: todo offset es en bytes; columnas en caracteres solo con `column_of` / `offset_at_column`.
- `Selection` / `Selections`: siempre al menos una, ordenadas y sin solapes (multi-cursor).
- `Edit` + `Buffer::apply`: lote de ediciones en coordenadas previas; `EditMap` traduce posiciones al texto nuevo.
- Transacciones: `begin_transaction` … `commit_transaction` forman un paso de undo/redo con las selecciones de antes y después.
- Historial en árbol: deshacer y editar abre una rama sin perder la anterior. `undo` / `redo` recorren la rama actual; `earlier` / `later` recorren todos los estados por `HistorySeq`, en orden cronológico.
- Grupos de undo: entre `begin_undo_group` y `end_undo_group` las transacciones se suman en un solo paso, como los cambios de una macro de vim. Moverse por el historial corta el grupo.
- `LineAnchor`: posición que sigue a su línea como una mark de vim (`mark_adjust`). Se corre cuando se insertan o borran líneas antes, conserva su columna en bytes con ediciones en su línea, se mueve con su texto si su línea se une a la anterior y se pierde si se borra su línea. Cada transacción guarda dónde estaban las anclas antes del cambio, y undo y redo las devuelven ahí.

No sabe nada de vim, archivos ni markdown.
