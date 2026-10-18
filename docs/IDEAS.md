# Ideas a futuro

Cosas que se quieren y que no son MVP. La pregunta para cada una no es "cuándo", sino **qué decisión de hoy la haría cara después**. Eso se anota en "Impacto en el diseño temprano".

## Plugins

Lo que quiere: un sistema de extensiones **sin tienda tipo VS Code**. Una extensión se instala desde la URL de su repo (git), y listo.

Opciones:

| Opción | Cómo es | A favor | En contra |
|---|---|---|---|
| **WASM** (`wasmtime` + component model / WIT) | El plugin se compila a `.wasm`; la app lo carga en un sandbox | Lo que usa Zed para sus extensiones. Aislado: un plugin no puede tumbar la app ni leer lo que no debe. Se escribe en Rust, Go, C, etc. | Más trabajo inicial; escribir un plugin requiere compilar |
| **Lua** (`mlua`) | Scripts `.lua`, como Neovim | Facilísimo de escribir y de iterar. Los usuarios de vim ya lo conocen | Sin sandbox real; performance menor; API más difícil de versionar |

Postura actual: **no decidir aún**. Probable: WASM para plugins "de verdad" y quizá Lua solo para config/scripts personales. Instalación: `:PluginInstall https://github.com/usuario/repo` clona, compila o descarga el `.wasm` del release, y lo registra en la config.

**Impacto en el diseño temprano (esto sí desde M0):**
- Todo lo que hace la app es una **acción con nombre** (`editor::delete_line`, `vault::open_file`; siempre snake_case). Los atajos, la paleta de comandos y los `:` comandos solo invocan acciones. Un plugin luego solo tiene que registrar acciones nuevas. Esto ya es el modelo de Zed que propone `03-core-architecture.md`.
- Eventos del núcleo explícitos (nota abierta, guardada, link creado…) en vez de llamadas directas entre módulos. Son los "hooks" a los que un plugin se suscribirá.
- El núcleo no depende de la GUI: un plugin habla con el núcleo, nunca con GPUI.

## Temas

Lo que quiere: personalizar colores, fuentes, algo de apariencia.

- Temas como archivos (TOML o JSON) en `~/.config/nota/themes/`, con hot reload al guardar.
- Instalables igual que los plugins: desde un repo.
- Posible importar temas de Zed o de Base16, que ya tienen catálogo enorme.

**Impacto en el diseño temprano:**
- **Ningún color ni tamaño de fuente hardcodeado** en la UI desde el primer commit. Todo sale de tokens de tema (`editor.background`, `text.muted`, `syntax.heading`). Arreglar esto después significa tocar cada vista.

## Historial en árbol (undotree)

Lo que quiere (17-oct-2026, al probar `u` / `ctrl-r` en el spike): algo tipo **undotree** de vim. Si deshaces y luego editas, la rama que deshiciste no se pierde: queda como otra rama del árbol y puedes volver a ella.

- ✅ **Hecho (17-oct-2026):** `History` en `nota-buffer` es un árbol de `Transaction` (ver 03 §1). `u` va al padre, `ctrl-r` al hijo visitado más recientemente, y `g-` / `g+` (`vim::earlier` / `vim::later`) recorren los estados en orden cronológico cruzando ramas, verificados contra nvim.
- Idea: vista del árbol en un panel, navegable con `j`/`k`, con vista previa del diff de cada estado.
- Idea: historial persistente entre sesiones (como `undofile`).

## Importador de Obsidian (y quizá de otros)

nota no busca ser compatible con Obsidian: tiene su propio formato y puede diferir donde haya algo más eficiente o mejor. Si algún día hace falta, un importador convierte el vault (links, frontmatter, adjuntos) al formato de nota.

## Pendientes de recoger

El autor ha usado varias apps de notas y tiene ideas de qué le gusta de cada una. Anotarlas aquí conforme salgan.
