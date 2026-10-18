# nota-vault

El vault en disco, sin GUI.

- `Vault::open(raíz)`: indexa la carpeta con `ignore` y la vigila con `notify`. Devuelve el `Vault` y un receptor de `VaultUpdate { changes, snapshot, links }`.
- `Vault::snapshot()`: el `VaultIndex` de este momento, sin bloquear (`ArcSwap`). `children(carpeta)` recorre el árbol ya ordenado (carpetas primero, nombre sin mayúsculas); `files()` da notas y adjuntos para el finder.
- `VaultChange`: `Created`, `Removed`, `Renamed` (cubre el contenido de una carpeta) y `Modified`. Un guardado atómico es `Modified`, no borrar y crear.
- `VaultPath`: ruta relativa con `/`, nunca sale de la raíz ni apunta a algo oculto.
- Operaciones: `create_note`, `create_folder`, `rename` (nunca pisa el destino) y `move_to_trash` (a `.trash/` del vault, como Obsidian). No pasan por enlaces simbólicos; el índice se entera por el watcher.
- `read` / `write_atomic`: leer una nota y guardarla sin dejarla nunca a medias.
- `walk_notes(raíz, visitante)`: recorre en paralelo las notas con las reglas del índice, para quien lea el disco por su cuenta (la búsqueda de texto de `nota-search`).
- `LinkIndex` (`Vault::links()`, y en cada `VaultUpdate`): los links de cada nota ya resueltos (`outgoing`), quién enlaza a cada archivo con la línea y su contexto (`backlinks`), y los links sin resolver o ambiguos. Se arma al abrir y el watcher lo mantiene al día; aplicar cambios da lo mismo que armarlo de cero.
- `LinkIndex::follow(nota, texto, offset)`: a dónde lleva el link bajo el cursor (archivo, heading o bloque), para `gd`; `LinkSubpath::locate(texto)` ubica el heading o el bloque en la nota destino.
- `LinkIndex::wikilink_target(nota, archivo)`: el destino más corto con que un `[[…]]` escrito en esa nota llega al archivo sin empatar, para el autocompletado de `[[`.
- `LinkIndex::rename_plan(de, a)`: las ediciones, con rangos exactos, que mantienen cada link apuntando a lo mismo tras renombrar o mover; cada link conserva su forma. `FileEdits::apply` comprueba que el archivo no cambió y devuelve también las ediciones que lo deshacen.
- `Vault::apply_rename(plan)` y `Vault::undo_rename(aplicado)`: reescriben los links en disco y renombran como una sola operación, sin escribir nada si un archivo cambió; deshacer no pisa lo que cambió después.

Ignora siempre lo oculto (`.obsidian/`, `.git/`, `.trash/`), respeta `.gitignore` e `.ignore` del vault y no sigue enlaces simbólicos. El markdown lo lee `nota-syntax`; aquí solo se indexa. Decisiones y cifras en `docs/research/03-core-architecture.md` §4.
