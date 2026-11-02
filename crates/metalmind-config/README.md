# metalmind-config

Rutas, `config.yaml` y temas de Metalmind, en YAML 1.2 y sin GUI: la UI convierte lo que sale de aquí a sus propios tipos. Las claves que puede escribir el usuario están en [`docs/config.md`](../../docs/config.md).

- `Paths`: dónde vive cada cosa (`config`, `data`, `cache`, `state`). `Paths::detect()` sale del sistema, `Paths::from_xdg` recibe el entorno para probar XDG sin tocar el del proceso y `Paths::under(raíz)` lo pone todo bajo una carpeta. De aquí salen `config_file`, `themes_dir`, `last_vault_file`, `vault_data` y `vault_state`.
- `Config::load(&Paths)`: `config.yaml` y el tema que elige, todo lo que se lee al arrancar. `Settings::load` es solo `config.yaml`, para quien no pinta nada, como la CLI.
- `Loaded<T>`: un valor y sus `Diagnostic`s, que pueden decir archivo, línea y columna. Nada de esto impide arrancar: un archivo roto se cambia por los defaults y el problema vuelve como diagnóstico.
- `Theme` y `Themes`: los temas embebidos (`BUILTIN_THEMES`, `themes/`) y los del usuario, con sus colores y medidas ya resueltos.
- `edit`: cambia una clave de un YAML del usuario sin reescribir el resto. Es texto a texto: quien llama lee el archivo y lo guarda de forma atómica. Cada edición se comprueba releyendo el resultado, y lo que no puede hacer sin tocar otra cosa es `EditError::Unsupported`.
- `Toggle` y `Settings::save_toggle`: las opciones que cambian desde la paleta y la clave donde se guarda cada una.
- `VaultConfig`: `<vault>/.metalmind/config.yaml`, que por ahora solo tiene las vistas guardadas.

Tests: `tests/config.rs` carga la config de principio a fin, `tests/themes.rs` y `tests/theme_tokens.rs` cargan los temas, también el de ejemplo de `docs/config.md`.
