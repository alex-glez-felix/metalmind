# nota-buffer

Texto editable de `nota`, sin GUI ni I/O.

- `Buffer`: rope (`ropey`) con líneas separadas solo por `\n`.
- `ByteOffset`: todo offset es en bytes; columnas en caracteres solo con `column_of` / `offset_at_column`.
- `Selection` / `Selections`: siempre al menos una, ordenadas y sin solapes (multi-cursor).
- `Edit` + `Buffer::apply`: lote de ediciones en coordenadas previas; `EditMap` traduce posiciones al texto nuevo.
- Transacciones: `begin_transaction` … `commit_transaction` forman un paso de undo/redo con las selecciones de antes y después.

No sabe nada de vim, archivos ni markdown.
