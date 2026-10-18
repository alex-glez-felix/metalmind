# metalmind-search

Búsqueda del vault, sin GUI.

## Rutas (finder)

- `FuzzyMatcher::rank(&Query, candidatos)`: búsqueda difusa con `nucleo-matcher` (el matcher de Helix). `FuzzyMatcher::paths()` da los bonus de rutas (empezar componente o palabra) y `FuzzyMatcher::text()` es para nombres sueltos, como los de la paleta. Devuelve `Ranked { index, score }` del mejor al peor; a igual puntaje gana el candidato más corto.
- `FuzzyMatcher::positions(&Query, candidato)`: offsets en **bytes** del inicio de cada grafema que coincide, para resaltarlos en la UI sin convertir nada. Va aparte porque es lo caro: la lista virtualizada solo los pide para las filas que pinta.
- Mayúsculas y acentos cuentan solo si el query los lleva (`nino` encuentra `niño`). El query vacío devuelve todo en el orden de entrada.

No sabe de dónde salen las rutas: el finder le pasa las de `metalmind_vault::VaultIndex::files`.

## Texto completo

- `TextSearcher::new(raíz).search(&TextQuery)`: un grep paralelo sobre las notas (`.md`) con las librerías de ripgrep (`grep-searcher`, `grep-regex`). Recorre el disco con `metalmind_vault::walk_notes`, las mismas reglas que el índice: `.gitignore` e `.ignore` del vault, sin ocultos ni enlaces simbólicos.
- `TextQuery::compile()?.find(texto)`: lo mismo sobre un texto en memoria, para una nota abierta con cambios sin guardar.
- `TextQuery { pattern, syntax, case, scope }`: `Syntax::Literal` o `Syntax::Regex` (la sintaxis de `regex`, una línea a la vez); `Case::Smart` (default, distingue solo si el patrón tiene mayúsculas; lo escapado no cuenta), `Sensitive` o `Insensitive`; `scope`, si está, limita la búsqueda a ese conjunto de `VaultPath`.
- Los resultados llegan por un `mpsc::Receiver<NoteMatches>` mientras se busca, nota por nota y en cualquier orden. Cada `LineMatch` trae la línea (desde 0), su offset en la nota, su texto sin el salto y cada coincidencia como rango en bytes dentro de ese texto (su `start` es la columna).
- Una búsqueda a la vez: `search` cancela la anterior y espera a sus hilos, así que por el receptor viejo no llega nada más. `cancel()` y soltar el `TextSearcher` también cancelan; soltar el receptor la corta en la próxima nota que coincida. Un patrón inválido devuelve error y deja seguir a la anterior.
- Solo se busca en lo que Metalmind puede abrir: una nota con un NUL o que no es UTF-8 no da resultados. `\r\n` no forma parte de la línea y `$` coincide antes de `\r`. El patrón vacío y las coincidencias vacías (`^`, `x*`) no dan resultados.

Decisiones y cifras en `docs/research/03-core-architecture.md` §4.
