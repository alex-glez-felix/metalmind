# metalmind-template

Plantillas de notas, la plantilla que pide cada tag y la nota del día, sin GUI. Cómo se usan está en [`docs/plantillas.md`](../../docs/plantillas.md); la decisión, en [06](../../docs/research/06-decisiones-de-fondo.md) §11.

Todo es puro salvo `Templates::load` y `capture`: la UI lee las plantillas una vez y pide borradores sin tocar el disco.

- `Templates::load(raíz, índice, carpeta)` lee las plantillas de la carpeta, que da `templates_folder`.
- `draft(&NewNote, &Templates, &TagGraph)` da el texto inicial de una nota nueva: la plantilla usada, dónde empezar a escribir y lo que salió distinto de lo pedido (`DraftProblem`). Con un problema, la nota se puede crear igual.
- `render(plantilla, Context)` llena `{{title}}`, `{{date}}`, `{{time}}`, `{{date:FORMATO}}` y `{{time:FORMATO}}` (`DateFormat`). Una variable mal escrita se deja tal cual y vuelve como `VariableProblem`.
- `tag_template(&TagGraph, tags)` dice qué plantilla piden los tags de una nota. Si piden plantillas distintas es un `TagChoice::Conflict` y no se usa ninguna: elegir una adivinaría. `tags_asking_for` es la consulta al revés.
- `with_tags(texto, tags)` agrega tags al frontmatter sin tocar lo demás. Si no puede, `texto` se queda igual.
- `daily_note` da la nota del día en la zona horaria local, o `None` si están apagadas.
- `capture` es `metalmind add` con plantilla: lee solo las carpetas de plantillas y de tags, no el vault entero, y crea la nota.
- `NotePath` pasa lo que se escribe como nombre de una nota a su ruta y su título.

Tests: tablas en cada módulo y `tests/load.rs` contra carpetas de verdad.
