# nota-syntax

La sintaxis de nota sobre el texto de una nota, sin GUI ni I/O. Adopta de Obsidian los wikilinks, los tags y el frontmatter, pero no busca ser compatible: donde su regla es peor se separa (ver `docs/research/03-core-architecture.md` §2). Todo rango es en bytes.

- `scan(&str) -> Scan`: links, tags, frontmatter y problemas recuperables, en orden de aparición.
- `Link`: wikilinks (`[[nota#Heading|alias]]`), embeds (`![[img.png|300]]`) y links markdown a archivos locales (`[texto](nota.md#H)`), con el rango del destino, la subruta (`Subpath::Heading` o `Subpath::Block`) y el alias. `[[#Heading]]` deja el destino vacío: la misma nota. `\|` también separa el alias, como se escribe en las tablas. `Link::text` decodifica los `%20` de un link markdown.
- `Tag`: `#tag` y `#a/b/c` al inicio de línea o tras un espacio, terminados en espacio o puntuación, nunca solo números (`is_valid_tag_name`): `PR #15` no es un tag.
- `Frontmatter`: el bloque `---` del principio con `tags`, `aliases` y `cssclasses` en forma de lista o de texto, leído como YAML 1.2 con `serde-saphyr` (el mismo crate que `nota-config`). Un YAML roto o un campo de otro tipo queda en `Scan::problems` y el resto se escanea igual.
- Nada se reconoce dentro de bloques cercados (``` y ~~~, también en listas y citas), código inline ni comentarios `%% %%`; un `%%` sin cierre comenta hasta el final.
- `resolve(target, rutas) -> Resolution`: sin distinguir mayúsculas, con o sin `.md`, por nombre o final de ruta (`carpeta/nota`); gana la de menos carpetas, un empate lo rompe la única con las mismas mayúsculas y si no, es `Ambiguous`.
- Lineal en el tamaño de la nota, también con corchetes o backticks sin cerrar.

No sabe de bloques de markdown (listas, citas, código indentado) ni de tree-sitter, que entra en M4. Tampoco de `^block-id`, headings ni aliases del vault al resolver: eso es del índice.
