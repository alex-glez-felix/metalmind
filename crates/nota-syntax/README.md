# nota-syntax

La sintaxis propia de Obsidian sobre el texto de una nota, sin GUI ni I/O. Todo rango es en bytes.

- `scan(&str) -> Scan`: links, tags, frontmatter y problemas recuperables, en orden de aparición.
- `Link`: wikilinks (`[[nota#Heading|alias]]`), embeds (`![[img.png|300]]`) y links markdown a archivos locales (`[texto](nota.md#H)`), con el rango del destino, la subruta (`Subpath::Heading` o `Subpath::Block`) y el alias. `[[#Heading]]` deja el destino vacío: la misma nota. `\|` también separa el alias, como se escribe en las tablas. `Link::text` decodifica los `%20` de un link markdown.
- `Tag`: `#tag` y `#a/b/c` al inicio de línea o tras un espacio, terminados en espacio o puntuación, nunca solo números (`is_valid_tag_name`).
- `Frontmatter`: el bloque `---` del principio con `tags`, `aliases` y `cssclasses` en forma de lista o de texto (y sus nombres viejos `tag`, `alias`, `cssclass`), leído con `yaml-rust2`. Un YAML roto queda en `Scan::problems` y el resto se escanea igual.
- Nada se reconoce dentro de bloques cercados (``` y ~~~, también en listas y citas), código inline ni comentarios `%% %%`; un `%%` sin cierre comenta hasta el final.
- `resolve(target, rutas) -> Resolution`: como Obsidian, sin distinguir mayúsculas, con o sin `.md`, por nombre o final de ruta (`carpeta/nota`); gana la de menos carpetas y un empate es `Ambiguous`.
- Lineal en el tamaño de la nota, también con corchetes o backticks sin cerrar.

No sabe de bloques de markdown (listas, citas, código indentado) ni de tree-sitter, que entra en M4. Tampoco de `^block-id`, headings ni aliases del vault al resolver: eso es del índice.
