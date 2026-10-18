# nota-vault

El vault en disco, sin GUI.

- `Vault::open(raíz)`: indexa la carpeta con `ignore` y la vigila con `notify`. Devuelve el `Vault` y un receptor de `VaultUpdate { changes, snapshot }`.
- `Vault::snapshot()`: el `VaultIndex` de este momento, sin bloquear (`ArcSwap`). `children(carpeta)` recorre el árbol ya ordenado (carpetas primero, nombre sin mayúsculas); `files()` da notas y adjuntos para el finder.
- `VaultChange`: `Created`, `Removed`, `Renamed` (cubre el contenido de una carpeta) y `Modified`. Un guardado atómico es `Modified`, no borrar y crear.
- `VaultPath`: ruta relativa con `/`, nunca sale de la raíz ni apunta a algo oculto.
- Operaciones: `create_note`, `create_folder`, `rename` (nunca pisa el destino) y `move_to_trash` (a `.trash/` del vault, como Obsidian). No pasan por enlaces simbólicos; el índice se entera por el watcher.
- `read` / `write_atomic`: leer una nota y guardarla sin dejarla nunca a medias.
- `walk_notes(raíz, visitante)`: recorre en paralelo las notas con las reglas del índice, para quien lea el disco por su cuenta (la búsqueda de texto de `nota-search`).

Ignora siempre lo oculto (`.obsidian/`, `.git/`, `.trash/`), respeta `.gitignore` e `.ignore` del vault y no sigue enlaces simbólicos. No sabe de markdown ni de links: eso llega en M3. Decisiones y cifras en `docs/research/03-core-architecture.md` §4.
