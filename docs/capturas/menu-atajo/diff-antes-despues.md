# Capturas del menú del árbol (#109): `main` contra esta rama

`scripts/capturas.sh` en Xvfb con lavapipe: primero `main` (`e81ab50`), después esta rama con `--antes`. De las 132 capturas solo cambian las ocho del menú contextual: «Abrir a la derecha» ya no queda pegada a `ctrl-v` y el menú crece hasta su fila más larga. `formulas-bloque-oscuro-1920` cambia 7 píxeles del borde de una fórmula, el mismo ruido de `main` contra `main` que se ve en [theme-desde-config](../theme-desde-config/); no va aquí.

Aquí van las ocho de la rama, las de `main` (`antes-…`) y el diff (`diff-…`).

| Captura | Píxeles distintos |
|---|---|
| editor-claro-1280 | 0 |
| editor-final-claro-1280 | 0 |
| arbol-claro-1280 | 0 |
| menu-claro-1280 | 2863 (`diff-menu-claro-1280.png`) |
| menu-borde-claro-1280 | 2925 (`diff-menu-borde-claro-1280.png`) |
| split-claro-1280 | 0 |
| linea-de-comandos-claro-1280 | 0 |
| finder-claro-1280 | 0 |
| finder-filtrado-claro-1280 | 0 |
| finder-ruta-larga-claro-1280 | 0 |
| finder-vacio-claro-1280 | 0 |
| buscar-claro-1280 | 0 |
| buscar-vacio-claro-1280 | 0 |
| buscar-cjk-claro-1280 | 0 |
| paleta-claro-1280 | 0 |
| paleta-final-claro-1280 | 0 |
| paleta-save-claro-1280 | 0 |
| paleta-focus-claro-1280 | 0 |
| paleta-workspace-claro-1280 | 0 |
| paleta-pane-claro-1280 | 0 |
| paleta-panel-claro-1280 | 0 |
| paleta-editor-claro-1280 | 0 |
| marks-claro-1280 | 0 |
| marks-nota-larga-claro-1280 | 0 |
| marks-borde-derecho-claro-1280 | 0 |
| listas-claro-1280 | 0 |
| listas-casilla-claro-1280 | 0 |
| formulas-claro-1280 | 0 |
| formulas-bloque-claro-1280 | 0 |
| formulas-larga-claro-1280 | 0 |
| autoguardado-guardado-claro-1280 | 0 |
| autoguardado-sin-guardar-claro-1280 | 0 |
| autoguardado-cambio-en-disco-claro-1280 | 0 |
| editor-claro-1920 | 0 |
| editor-final-claro-1920 | 0 |
| arbol-claro-1920 | 0 |
| menu-claro-1920 | 2863 (`diff-menu-claro-1920.png`) |
| menu-borde-claro-1920 | 2980 (`diff-menu-borde-claro-1920.png`) |
| split-claro-1920 | 0 |
| linea-de-comandos-claro-1920 | 0 |
| finder-claro-1920 | 0 |
| finder-filtrado-claro-1920 | 0 |
| finder-ruta-larga-claro-1920 | 0 |
| finder-vacio-claro-1920 | 0 |
| buscar-claro-1920 | 0 |
| buscar-vacio-claro-1920 | 0 |
| buscar-cjk-claro-1920 | 0 |
| paleta-claro-1920 | 0 |
| paleta-final-claro-1920 | 0 |
| paleta-save-claro-1920 | 0 |
| paleta-focus-claro-1920 | 0 |
| paleta-workspace-claro-1920 | 0 |
| paleta-pane-claro-1920 | 0 |
| paleta-panel-claro-1920 | 0 |
| paleta-editor-claro-1920 | 0 |
| marks-claro-1920 | 0 |
| marks-nota-larga-claro-1920 | 0 |
| marks-borde-derecho-claro-1920 | 0 |
| listas-claro-1920 | 0 |
| listas-casilla-claro-1920 | 0 |
| formulas-claro-1920 | 0 |
| formulas-bloque-claro-1920 | 0 |
| formulas-larga-claro-1920 | 0 |
| autoguardado-guardado-claro-1920 | 0 |
| autoguardado-sin-guardar-claro-1920 | 0 |
| autoguardado-cambio-en-disco-claro-1920 | 0 |
| editor-oscuro-1280 | 0 |
| editor-final-oscuro-1280 | 0 |
| arbol-oscuro-1280 | 0 |
| menu-oscuro-1280 | 4823 (`diff-menu-oscuro-1280.png`) |
| menu-borde-oscuro-1280 | 4823 (`diff-menu-borde-oscuro-1280.png`) |
| split-oscuro-1280 | 0 |
| linea-de-comandos-oscuro-1280 | 0 |
| finder-oscuro-1280 | 0 |
| finder-filtrado-oscuro-1280 | 0 |
| finder-ruta-larga-oscuro-1280 | 0 |
| finder-vacio-oscuro-1280 | 0 |
| buscar-oscuro-1280 | 0 |
| buscar-vacio-oscuro-1280 | 0 |
| buscar-cjk-oscuro-1280 | 0 |
| paleta-oscuro-1280 | 0 |
| paleta-final-oscuro-1280 | 0 |
| paleta-save-oscuro-1280 | 0 |
| paleta-focus-oscuro-1280 | 0 |
| paleta-workspace-oscuro-1280 | 0 |
| paleta-pane-oscuro-1280 | 0 |
| paleta-panel-oscuro-1280 | 0 |
| paleta-editor-oscuro-1280 | 0 |
| marks-oscuro-1280 | 0 |
| marks-nota-larga-oscuro-1280 | 0 |
| marks-borde-derecho-oscuro-1280 | 0 |
| listas-oscuro-1280 | 0 |
| listas-casilla-oscuro-1280 | 0 |
| formulas-oscuro-1280 | 0 |
| formulas-bloque-oscuro-1280 | 0 |
| formulas-larga-oscuro-1280 | 0 |
| autoguardado-guardado-oscuro-1280 | 0 |
| autoguardado-sin-guardar-oscuro-1280 | 0 |
| autoguardado-cambio-en-disco-oscuro-1280 | 0 |
| editor-oscuro-1920 | 0 |
| editor-final-oscuro-1920 | 0 |
| arbol-oscuro-1920 | 0 |
| menu-oscuro-1920 | 4823 (`diff-menu-oscuro-1920.png`) |
| menu-borde-oscuro-1920 | 4823 (`diff-menu-borde-oscuro-1920.png`) |
| split-oscuro-1920 | 0 |
| linea-de-comandos-oscuro-1920 | 0 |
| finder-oscuro-1920 | 0 |
| finder-filtrado-oscuro-1920 | 0 |
| finder-ruta-larga-oscuro-1920 | 0 |
| finder-vacio-oscuro-1920 | 0 |
| buscar-oscuro-1920 | 0 |
| buscar-vacio-oscuro-1920 | 0 |
| buscar-cjk-oscuro-1920 | 0 |
| paleta-oscuro-1920 | 0 |
| paleta-final-oscuro-1920 | 0 |
| paleta-save-oscuro-1920 | 0 |
| paleta-focus-oscuro-1920 | 0 |
| paleta-workspace-oscuro-1920 | 0 |
| paleta-pane-oscuro-1920 | 0 |
| paleta-panel-oscuro-1920 | 0 |
| paleta-editor-oscuro-1920 | 0 |
| marks-oscuro-1920 | 0 |
| marks-nota-larga-oscuro-1920 | 0 |
| marks-borde-derecho-oscuro-1920 | 0 |
| listas-oscuro-1920 | 0 |
| listas-casilla-oscuro-1920 | 0 |
| formulas-oscuro-1920 | 0 |
| formulas-bloque-oscuro-1920 | 7 (`diff-formulas-bloque-oscuro-1920.png`) |
| formulas-larga-oscuro-1920 | 0 |
| autoguardado-guardado-oscuro-1920 | 0 |
| autoguardado-sin-guardar-oscuro-1920 | 0 |
| autoguardado-cambio-en-disco-oscuro-1920 | 0 |
