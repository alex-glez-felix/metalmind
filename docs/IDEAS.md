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

- Temas como archivos (TOML o JSON) en `~/.config/metalmind/themes/`, con hot reload al guardar.
- Instalables igual que los plugins: desde un repo.
- Posible importar temas de Zed o de Base16, que ya tienen catálogo enorme.

**Impacto en el diseño temprano:**
- **Ningún color ni tamaño de fuente hardcodeado** en la UI desde el primer commit. Todo sale de tokens de tema (`editor.background`, `text.muted`, `syntax.heading`). Arreglar esto después significa tocar cada vista.

## Historial en árbol (undotree)

Lo que quiere (17-oct-2026, al probar `u` / `ctrl-r` en el spike): algo tipo **undotree** de vim. Si deshaces y luego editas, la rama que deshiciste no se pierde: queda como otra rama del árbol y puedes volver a ella.

- ✅ **Hecho (17-oct-2026):** `History` en `metalmind-buffer` es un árbol de `Transaction` (ver 03 §1). `u` va al padre, `ctrl-r` al hijo visitado más recientemente, y `g-` / `g+` (`vim::earlier` / `vim::later`) recorren los estados en orden cronológico cruzando ramas, verificados contra nvim.
- Idea: vista del árbol en un panel, navegable con `j`/`k`, con vista previa del diff de cada estado.
- Idea: historial persistente entre sesiones (como `undofile`).

## Tags

Los tags son el eje de organización desde M2.5 ([05](research/05-tags.md)). Lo que quedó fuera de M2.5:

- **Vistas guardadas**: decididas en [06](research/06-decisiones-de-fondo.md#10-vistas-guardadas): una consulta de tags con nombre (`gs` = `#gs -#archivado`) que aparece en el panel junto a los tags. Falta decidir el archivo exacto donde se guardan.
- Consultas más ricas: "o", paréntesis, comodines (`#2024-*`) y "solo este tag, sin descendientes". Juntas con las vistas guardadas, que es donde una consulta larga vale la pena.
- Recordar el focus al reabrir el vault.
- Descargar sets de íconos desde la UI; en M2.5 se agregan dejando los SVG en `<config>/icons/<set>/` (05 §7). Color, ícono (emoji, Tabler y sets propios), alias, padres y la página que describe un tag ya entran en M2.5 ([05 §7](research/05-tags.md#7-páginas-de-tag)).
- Sugerir tags a partir del contenido de la nota.
- Que el panel de tags siga al buffer sin guardar, no solo al disco.

**Impacto en el diseño temprano:**
- La consulta es un tipo (`TagQuery`) con parseo puro y tabla de casos: agregar "o" o darle nombre a una consulta no toca a quien la usa.
- Renombrar tags y renombrar notas (M3) comparten el motor de refactorización del vault con vista previa y deshacer.

## Decididas, sin fecha

Ver [06 — Decisiones de fondo](research/06-decisiones-de-fondo.md), que dice qué restringe cada una desde ya.

- **Celular**: quizá una app simple de captura rápida; la principal es la de escritorio ([06 §2](research/06-decisiones-de-fondo.md#2-celular)).
- **Cifrado de notas**: deseable, sin fecha; nada derivado guarda en claro una nota cifrada ([06 §4](research/06-decisiones-de-fondo.md#4-cifrado-de-notas)).
- **Grafo**: deseable, prioridad baja ([06 §11](research/06-decisiones-de-fondo.md#11-baratas-sin-restricción-de-arquitectura)).
- **Plantillas de notas** y **daily notes** (apagadas por defecto): el núcleo está en `metalmind-template`; faltan la UI y la CLI ([06 §11](research/06-decisiones-de-fondo.md#11-baratas-sin-restricción-de-arquitectura)).

## Importador de Obsidian (y quizá de otros)

Metalmind no busca ser compatible con Obsidian: tiene su propio formato y puede diferir donde haya algo más eficiente o mejor. Si algún día hace falta, un importador convierte el vault (links, frontmatter, adjuntos) al formato de Metalmind.

## Pendientes de recoger

El autor ha usado varias apps de notas y tiene ideas de qué le gusta de cada una. Anotarlas aquí conforme salgan.
