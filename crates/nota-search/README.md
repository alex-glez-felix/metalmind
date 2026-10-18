# nota-search

Búsqueda del vault, sin GUI ni I/O.

- `PathMatcher::search(query, candidatos)`: búsqueda difusa de rutas con `nucleo-matcher` (el matcher de Helix), con los bonus de rutas (empezar componente o palabra). Devuelve `PathMatch { index, score, positions }` del mejor al peor; a igual puntaje gana la ruta más corta.
- `positions` son offsets en **bytes** del inicio de cada grafema que coincide, para resaltarlos en la UI sin convertir nada.
- Mayúsculas y acentos cuentan solo si el query los lleva (`nino` encuentra `niño`). El query vacío devuelve todo en el orden de entrada.

No sabe de dónde salen las rutas: el finder le pasa las de `nota_vault::VaultIndex::files`. La búsqueda full-text llegará aquí después (03 §4).
