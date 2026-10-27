# Capturas del soft-wrap (#175, #138)

`scripts/capturas.sh` en `main` (antes, sin partir las líneas) y en la rama de #183, con `--antes`. Cambian las pantallas del editor con líneas más anchas que la ventana: ahora se parten en filas. En `formulas-larga`, la línea de las fórmulas rotas ocupa varias filas y el tooltip sale sobre la fila del ratón. `autocompletado-alias-largo` es la escena de #138: escribiendo al final de una línea más ancha que el panel, el cursor (`7:229`) se ve porque la línea se parte. Las escenas nuevas, al final de `SCENES`: `wrap-seleccion` (búsqueda y selección que cruzan filas), `sin-wrap` y `sin-wrap-palabra` (`:set nowrap`, la vista corrida a lo ancho hasta el cursor). En `antes/`, `despues/` y `diff/` van las que cambian; la tabla es de todas. Las de 1920 se tomaron con más tiempo por caso (`terminate-after = 15`): en una máquina sin GPU, con lavapipe, el caso entero pasa de los 180 s del perfil, también en `main`.

| Captura | Píxeles distintos |
|---|---|
| editor-claro-1280 | 0 |
| editor-final-claro-1280 | 30824 (`diff-editor-final-claro-1280.png`) |
| arbol-claro-1280 | 0 |
| menu-claro-1280 | 0 |
| menu-borde-claro-1280 | 0 |
| split-claro-1280 | 203718 (`diff-split-claro-1280.png`) |
| linea-de-comandos-claro-1280 | 0 |
| finder-claro-1280 | 0 |
| finder-filtrado-claro-1280 | 0 |
| finder-ruta-larga-claro-1280 | 0 |
| finder-vacio-claro-1280 | 0 |
| finder-tags-claro-1280 | 0 |
| finder-tags-inexistentes-claro-1280 | 0 |
| finder-buzon-claro-1280 | 0 |
| buscar-claro-1280 | 0 |
| buscar-vacio-claro-1280 | 0 |
| buscar-escrito-largo-claro-1280 | 0 |
| buscar-cjk-claro-1280 | 0 |
| paleta-claro-1280 | 0 |
| paleta-final-claro-1280 | 0 |
| paleta-save-claro-1280 | 0 |
| paleta-focus-claro-1280 | 0 |
| paleta-workspace-claro-1280 | 0 |
| paleta-pane-claro-1280 | 0 |
| paleta-panel-claro-1280 | 0 |
| paleta-editor-claro-1280 | 3154 (`diff-paleta-editor-claro-1280.png`) |
| marks-claro-1280 | 0 |
| marks-nota-larga-claro-1280 | 35664 (`diff-marks-nota-larga-claro-1280.png`) |
| marks-borde-derecho-claro-1280 | 93866 (`diff-marks-borde-derecho-claro-1280.png`) |
| listas-claro-1280 | 0 |
| listas-casilla-claro-1280 | 0 |
| formulas-claro-1280 | 12607 (`diff-formulas-claro-1280.png`) |
| formulas-bloque-claro-1280 | 39877 (`diff-formulas-bloque-claro-1280.png`) |
| formulas-larga-claro-1280 | 140530 (`diff-formulas-larga-claro-1280.png`) |
| autoguardado-guardado-claro-1280 | 0 |
| autoguardado-sin-guardar-claro-1280 | 0 |
| autoguardado-cambio-en-disco-claro-1280 | 0 |
| tags-claro-1280 | 0 |
| tags-abierto-claro-1280 | 0 |
| tags-buzon-claro-1280 | 0 |
| tags-menu-borde-claro-1280 | 0 |
| autocompletado-claro-1280 | 0 |
| autocompletado-alias-claro-1280 | 0 |
| autocompletado-elegir-claro-1280 | 0 |
| autocompletado-aceptado-claro-1280 | 0 |
| autocompletado-borde-claro-1280 | 239146 (`diff-autocompletado-borde-claro-1280.png`) |
| autocompletado-alias-largo-claro-1280 | 56494 (`diff-autocompletado-alias-largo-claro-1280.png`) |
| autocompletado-panel-bajo-claro-1280 | 50888 (`diff-autocompletado-panel-bajo-claro-1280.png`) |
| tags-menu-tag-claro-1280 | 11751 (`diff-tags-menu-tag-claro-1280.png`) |
| tags-renombrar-pregunta-claro-1280 | 11991 (`diff-tags-renombrar-pregunta-claro-1280.png`) |
| renombrar-tag-claro-1280 | 9036 (`diff-renombrar-tag-claro-1280.png`) |
| renombrar-tag-quitado-claro-1280 | 9036 (`diff-renombrar-tag-quitado-claro-1280.png`) |
| renombrar-tag-final-claro-1280 | 9036 (`diff-renombrar-tag-final-claro-1280.png`) |
| fusionar-tag-bloqueado-claro-1280 | 9036 (`diff-fusionar-tag-bloqueado-claro-1280.png`) |
| renombrar-tag-aplicado-claro-1280 | 11991 (`diff-renombrar-tag-aplicado-claro-1280.png`) |
| renombrar-tag-deshecho-claro-1280 | 11991 (`diff-renombrar-tag-deshecho-claro-1280.png`) |
| tags-menu-nueva-nota-claro-1280 | 11991 (`diff-tags-menu-nueva-nota-claro-1280.png`) |
| tags-nueva-nota-claro-1280 | 9036 (`diff-tags-nueva-nota-claro-1280.png`) |
| tags-nueva-nota-creada-claro-1280 | 8982 (`diff-tags-nueva-nota-creada-claro-1280.png`) |
| plantillas-claro-1280 | 814 (`diff-plantillas-claro-1280.png`) |
| plantillas-filtradas-claro-1280 | 4229 (`diff-plantillas-filtradas-claro-1280.png`) |
| plantillas-vacio-claro-1280 | 6728 (`diff-plantillas-vacio-claro-1280.png`) |
| plantilla-nombre-claro-1280 | 6728 (`diff-plantilla-nombre-claro-1280.png`) |
| plantilla-nombre-tomado-claro-1280 | 6728 (`diff-plantilla-nombre-tomado-claro-1280.png`) |
| plantilla-nombre-largo-claro-1280 | 6728 (`diff-plantilla-nombre-largo-claro-1280.png`) |
| plantilla-nombre-largo-inicio-claro-1280 | 6728 (`diff-plantilla-nombre-largo-inicio-claro-1280.png`) |
| plantilla-creada-claro-1280 | 46095 (`diff-plantilla-creada-claro-1280.png`) |
| nota-del-dia-claro-1280 | 0 |
| tags-menu-completo-claro-1280 | 0 |
| tags-alias-pregunta-claro-1280 | 0 |
| tags-alias-largos-claro-1280 | 0 |
| tags-alias-largos-inicio-claro-1280 | 0 |
| tags-alias-vista-previa-claro-1280 | 0 |
| tags-alias-aplicado-claro-1280 | 0 |
| tags-icono-invalido-claro-1280 | 0 |
| tags-pagina-claro-1280 | 7311 (`diff-tags-pagina-claro-1280.png`) |
| links-autocompletado-claro-1280 | 107415 (`diff-links-autocompletado-claro-1280.png`) |
| links-autocompletado-filtrado-claro-1280 | 44300 (`diff-links-autocompletado-filtrado-claro-1280.png`) |
| links-autocompletado-empate-claro-1280 | 61013 (`diff-links-autocompletado-empate-claro-1280.png`) |
| links-autocompletado-aceptado-claro-1280 | 14280 (`diff-links-autocompletado-aceptado-claro-1280.png`) |
| links-autocompletado-borde-claro-1280 | 145397 (`diff-links-autocompletado-borde-claro-1280.png`) |
| links-autocompletado-ruta-larga-claro-1280 | 121563 (`diff-links-autocompletado-ruta-larga-claro-1280.png`) |
| tabla-desalineada-claro-1280 | 29608 (`diff-tabla-desalineada-claro-1280.png`) |
| tabla-paleta-claro-1280 | 29608 (`diff-tabla-paleta-claro-1280.png`) |
| tabla-alineada-claro-1280 | 62189 (`diff-tabla-alineada-claro-1280.png`) |
| tabla-larga-alineada-claro-1280 | 0 |
| focus-completar-carpetas-claro-1280 | no hay captura de antes |
| focus-completar-tags-claro-1280 | no hay captura de antes |
| focus-completar-espacios-claro-1280 | no hay captura de antes |
| focus-carpeta-con-espacios-claro-1280 | no hay captura de antes |
| focus-tags-claro-1280 | no hay captura de antes |
| focus-carpetas-claro-1280 | no hay captura de antes |
| focus-menu-claro-1280 | no hay captura de antes |
| focus-finder-claro-1280 | no hay captura de antes |
| focus-buscar-claro-1280 | no hay captura de antes |
| focus-largo-claro-1280 | no hay captura de antes |
| focus-largo-finder-claro-1280 | no hay captura de antes |
| focus-vacio-claro-1280 | no hay captura de antes |
| focus-finder-vacio-claro-1280 | no hay captura de antes |
| focus-desde-el-panel-claro-1280 | no hay captura de antes |
| focus-menu-salir-claro-1280 | no hay captura de antes |
| sin-focus-claro-1280 | no hay captura de antes |
| backlinks-claro-1280 | no hay captura de antes |
| backlinks-final-claro-1280 | no hay captura de antes |
| backlinks-plegado-claro-1280 | no hay captura de antes |
| backlinks-menu-borde-claro-1280 | no hay captura de antes |
| backlinks-abierto-claro-1280 | no hay captura de antes |
| sin-destino-claro-1280 | no hay captura de antes |
| sin-destino-filtrado-claro-1280 | no hay captura de antes |
| sin-destino-vacio-claro-1280 | no hay captura de antes |
| sin-destino-abierto-claro-1280 | no hay captura de antes |
| autocompletado-angosto-claro-1280 | no hay captura de antes |
| links-autocompletado-angosto-claro-1280 | no hay captura de antes |
| wrap-seleccion-claro-1280 | no hay captura de antes |
| sin-wrap-claro-1280 | no hay captura de antes |
| sin-wrap-palabra-claro-1280 | no hay captura de antes |
| editor-claro-1920 | 51390 (`diff-editor-claro-1920.png`) |
| editor-final-claro-1920 | 38160 (`diff-editor-final-claro-1920.png`) |
| arbol-claro-1920 | 51390 (`diff-arbol-claro-1920.png`) |
| menu-claro-1920 | 51390 (`diff-menu-claro-1920.png`) |
| menu-borde-claro-1920 | 43309 (`diff-menu-borde-claro-1920.png`) |
| split-claro-1920 | 228884 (`diff-split-claro-1920.png`) |
| linea-de-comandos-claro-1920 | 51390 (`diff-linea-de-comandos-claro-1920.png`) |
| finder-claro-1920 | 51390 (`diff-finder-claro-1920.png`) |
| finder-filtrado-claro-1920 | 51390 (`diff-finder-filtrado-claro-1920.png`) |
| finder-ruta-larga-claro-1920 | 51390 (`diff-finder-ruta-larga-claro-1920.png`) |
| finder-vacio-claro-1920 | 304723 (`diff-finder-vacio-claro-1920.png`) |
| finder-tags-claro-1920 | 305867 (`diff-finder-tags-claro-1920.png`) |
| finder-tags-inexistentes-claro-1920 | 51390 (`diff-finder-tags-inexistentes-claro-1920.png`) |
| finder-buzon-claro-1920 | 70002 (`diff-finder-buzon-claro-1920.png`) |
| buscar-claro-1920 | 305283 (`diff-buscar-claro-1920.png`) |
| buscar-vacio-claro-1920 | 51390 (`diff-buscar-vacio-claro-1920.png`) |
| buscar-escrito-largo-claro-1920 | 51390 (`diff-buscar-escrito-largo-claro-1920.png`) |
| buscar-cjk-claro-1920 | 115832 (`diff-buscar-cjk-claro-1920.png`) |
| paleta-claro-1920 | 253280 (`diff-paleta-claro-1920.png`) |
| paleta-final-claro-1920 | 51390 (`diff-paleta-final-claro-1920.png`) |
| paleta-save-claro-1920 | 51390 (`diff-paleta-save-claro-1920.png`) |
| paleta-focus-claro-1920 | 167294 (`diff-paleta-focus-claro-1920.png`) |
| paleta-workspace-claro-1920 | 51390 (`diff-paleta-workspace-claro-1920.png`) |
| paleta-pane-claro-1920 | 51390 (`diff-paleta-pane-claro-1920.png`) |
| paleta-panel-claro-1920 | 51390 (`diff-paleta-panel-claro-1920.png`) |
| paleta-editor-claro-1920 | 54544 (`diff-paleta-editor-claro-1920.png`) |
| marks-claro-1920 | 51550 (`diff-marks-claro-1920.png`) |
| marks-nota-larga-claro-1920 | 43259 (`diff-marks-nota-larga-claro-1920.png`) |
| marks-borde-derecho-claro-1920 | 125776 (`diff-marks-borde-derecho-claro-1920.png`) |
| listas-claro-1920 | 0 |
| listas-casilla-claro-1920 | 0 |
| formulas-claro-1920 | 18146 (`diff-formulas-claro-1920.png`) |
| formulas-bloque-claro-1920 | 0 |
| formulas-larga-claro-1920 | 320042 (`diff-formulas-larga-claro-1920.png`) |
| autoguardado-guardado-claro-1920 | 0 |
| autoguardado-sin-guardar-claro-1920 | 0 |
| autoguardado-cambio-en-disco-claro-1920 | 0 |
| tags-claro-1920 | 0 |
| tags-abierto-claro-1920 | 0 |
| tags-buzon-claro-1920 | 0 |
| tags-menu-borde-claro-1920 | 0 |
| autocompletado-claro-1920 | 0 |
| autocompletado-alias-claro-1920 | 0 |
| autocompletado-elegir-claro-1920 | 0 |
| autocompletado-aceptado-claro-1920 | 0 |
| autocompletado-borde-claro-1920 | 280680 (`diff-autocompletado-borde-claro-1920.png`) |
| autocompletado-alias-largo-claro-1920 | 84818 (`diff-autocompletado-alias-largo-claro-1920.png`) |
| autocompletado-panel-bajo-claro-1920 | 7277 (`diff-autocompletado-panel-bajo-claro-1920.png`) |
| tags-menu-tag-claro-1920 | 16918 (`diff-tags-menu-tag-claro-1920.png`) |
| tags-renombrar-pregunta-claro-1920 | 17305 (`diff-tags-renombrar-pregunta-claro-1920.png`) |
| renombrar-tag-claro-1920 | 11388 (`diff-renombrar-tag-claro-1920.png`) |
| renombrar-tag-quitado-claro-1920 | 11388 (`diff-renombrar-tag-quitado-claro-1920.png`) |
| renombrar-tag-final-claro-1920 | 11388 (`diff-renombrar-tag-final-claro-1920.png`) |
| fusionar-tag-bloqueado-claro-1920 | 11388 (`diff-fusionar-tag-bloqueado-claro-1920.png`) |
| renombrar-tag-aplicado-claro-1920 | 17305 (`diff-renombrar-tag-aplicado-claro-1920.png`) |
| renombrar-tag-deshecho-claro-1920 | 17305 (`diff-renombrar-tag-deshecho-claro-1920.png`) |
| tags-menu-nueva-nota-claro-1920 | 17305 (`diff-tags-menu-nueva-nota-claro-1920.png`) |
| tags-nueva-nota-claro-1920 | 13286 (`diff-tags-nueva-nota-claro-1920.png`) |
| tags-nueva-nota-creada-claro-1920 | 0 |
| plantillas-claro-1920 | 0 |
| plantillas-filtradas-claro-1920 | 0 |
| plantillas-vacio-claro-1920 | no hay captura de antes |
| plantilla-nombre-claro-1920 | no hay captura de antes |
| plantilla-nombre-tomado-claro-1920 | no hay captura de antes |
| plantilla-nombre-largo-claro-1920 | no hay captura de antes |
| plantilla-nombre-largo-inicio-claro-1920 | no hay captura de antes |
| plantilla-creada-claro-1920 | no hay captura de antes |
| nota-del-dia-claro-1920 | no hay captura de antes |
| tags-menu-completo-claro-1920 | no hay captura de antes |
| tags-alias-pregunta-claro-1920 | no hay captura de antes |
| tags-alias-largos-claro-1920 | no hay captura de antes |
| tags-alias-largos-inicio-claro-1920 | no hay captura de antes |
| tags-alias-vista-previa-claro-1920 | no hay captura de antes |
| tags-alias-aplicado-claro-1920 | no hay captura de antes |
| tags-icono-invalido-claro-1920 | no hay captura de antes |
| tags-pagina-claro-1920 | no hay captura de antes |
| links-autocompletado-claro-1920 | no hay captura de antes |
| links-autocompletado-filtrado-claro-1920 | no hay captura de antes |
| links-autocompletado-empate-claro-1920 | no hay captura de antes |
| links-autocompletado-aceptado-claro-1920 | no hay captura de antes |
| links-autocompletado-borde-claro-1920 | no hay captura de antes |
| links-autocompletado-ruta-larga-claro-1920 | no hay captura de antes |
| tabla-desalineada-claro-1920 | no hay captura de antes |
| tabla-paleta-claro-1920 | no hay captura de antes |
| tabla-alineada-claro-1920 | no hay captura de antes |
| tabla-larga-alineada-claro-1920 | no hay captura de antes |
| focus-completar-carpetas-claro-1920 | no hay captura de antes |
| focus-completar-tags-claro-1920 | no hay captura de antes |
| focus-completar-espacios-claro-1920 | no hay captura de antes |
| focus-carpeta-con-espacios-claro-1920 | no hay captura de antes |
| focus-tags-claro-1920 | no hay captura de antes |
| focus-carpetas-claro-1920 | no hay captura de antes |
| focus-menu-claro-1920 | no hay captura de antes |
| focus-finder-claro-1920 | no hay captura de antes |
| focus-buscar-claro-1920 | no hay captura de antes |
| focus-largo-claro-1920 | no hay captura de antes |
| focus-largo-finder-claro-1920 | no hay captura de antes |
| focus-vacio-claro-1920 | no hay captura de antes |
| focus-finder-vacio-claro-1920 | no hay captura de antes |
| focus-desde-el-panel-claro-1920 | no hay captura de antes |
| focus-menu-salir-claro-1920 | no hay captura de antes |
| sin-focus-claro-1920 | no hay captura de antes |
| backlinks-claro-1920 | no hay captura de antes |
| backlinks-final-claro-1920 | no hay captura de antes |
| backlinks-plegado-claro-1920 | no hay captura de antes |
| backlinks-menu-borde-claro-1920 | no hay captura de antes |
| backlinks-abierto-claro-1920 | no hay captura de antes |
| sin-destino-claro-1920 | no hay captura de antes |
| sin-destino-filtrado-claro-1920 | no hay captura de antes |
| sin-destino-vacio-claro-1920 | no hay captura de antes |
| sin-destino-abierto-claro-1920 | no hay captura de antes |
| autocompletado-angosto-claro-1920 | no hay captura de antes |
| links-autocompletado-angosto-claro-1920 | no hay captura de antes |
| wrap-seleccion-claro-1920 | no hay captura de antes |
| sin-wrap-claro-1920 | no hay captura de antes |
| sin-wrap-palabra-claro-1920 | no hay captura de antes |
| editor-oscuro-1280 | 0 |
| editor-final-oscuro-1280 | 30819 (`diff-editor-final-oscuro-1280.png`) |
| arbol-oscuro-1280 | 0 |
| menu-oscuro-1280 | 0 |
| menu-borde-oscuro-1280 | 0 |
| split-oscuro-1280 | 203696 (`diff-split-oscuro-1280.png`) |
| linea-de-comandos-oscuro-1280 | 0 |
| finder-oscuro-1280 | 0 |
| finder-filtrado-oscuro-1280 | 0 |
| finder-ruta-larga-oscuro-1280 | 0 |
| finder-vacio-oscuro-1280 | 0 |
| finder-tags-oscuro-1280 | 0 |
| finder-tags-inexistentes-oscuro-1280 | 0 |
| finder-buzon-oscuro-1280 | 0 |
| buscar-oscuro-1280 | 192928 (`diff-buscar-oscuro-1280.png`) |
| buscar-vacio-oscuro-1280 | 0 |
| buscar-escrito-largo-oscuro-1280 | 0 |
| buscar-cjk-oscuro-1280 | 0 |
| paleta-oscuro-1280 | 0 |
| paleta-final-oscuro-1280 | 0 |
| paleta-save-oscuro-1280 | 0 |
| paleta-focus-oscuro-1280 | 0 |
| paleta-workspace-oscuro-1280 | 0 |
| paleta-pane-oscuro-1280 | 0 |
| paleta-panel-oscuro-1280 | 0 |
| paleta-editor-oscuro-1280 | 3154 (`diff-paleta-editor-oscuro-1280.png`) |
| marks-oscuro-1280 | 0 |
| marks-nota-larga-oscuro-1280 | 50577 (`diff-marks-nota-larga-oscuro-1280.png`) |
| marks-borde-derecho-oscuro-1280 | 110618 (`diff-marks-borde-derecho-oscuro-1280.png`) |
| listas-oscuro-1280 | 0 |
| listas-casilla-oscuro-1280 | 0 |
| formulas-oscuro-1280 | 12617 (`diff-formulas-oscuro-1280.png`) |
| formulas-bloque-oscuro-1280 | 39518 (`diff-formulas-bloque-oscuro-1280.png`) |
| formulas-larga-oscuro-1280 | 155402 (`diff-formulas-larga-oscuro-1280.png`) |
| autoguardado-guardado-oscuro-1280 | 0 |
| autoguardado-sin-guardar-oscuro-1280 | 0 |
| autoguardado-cambio-en-disco-oscuro-1280 | 0 |
| tags-oscuro-1280 | 0 |
| tags-abierto-oscuro-1280 | 0 |
| tags-buzon-oscuro-1280 | 0 |
| tags-menu-borde-oscuro-1280 | 0 |
| autocompletado-oscuro-1280 | 0 |
| autocompletado-alias-oscuro-1280 | 0 |
| autocompletado-elegir-oscuro-1280 | 0 |
| autocompletado-aceptado-oscuro-1280 | 0 |
| autocompletado-borde-oscuro-1280 | 284670 (`diff-autocompletado-borde-oscuro-1280.png`) |
| autocompletado-alias-largo-oscuro-1280 | 66552 (`diff-autocompletado-alias-largo-oscuro-1280.png`) |
| autocompletado-panel-bajo-oscuro-1280 | 50920 (`diff-autocompletado-panel-bajo-oscuro-1280.png`) |
| tags-menu-tag-oscuro-1280 | 11755 (`diff-tags-menu-tag-oscuro-1280.png`) |
| tags-renombrar-pregunta-oscuro-1280 | 11995 (`diff-tags-renombrar-pregunta-oscuro-1280.png`) |
| renombrar-tag-oscuro-1280 | 9041 (`diff-renombrar-tag-oscuro-1280.png`) |
| renombrar-tag-quitado-oscuro-1280 | 9041 (`diff-renombrar-tag-quitado-oscuro-1280.png`) |
| renombrar-tag-final-oscuro-1280 | 9041 (`diff-renombrar-tag-final-oscuro-1280.png`) |
| fusionar-tag-bloqueado-oscuro-1280 | 9041 (`diff-fusionar-tag-bloqueado-oscuro-1280.png`) |
| renombrar-tag-aplicado-oscuro-1280 | 11995 (`diff-renombrar-tag-aplicado-oscuro-1280.png`) |
| renombrar-tag-deshecho-oscuro-1280 | 11995 (`diff-renombrar-tag-deshecho-oscuro-1280.png`) |
| tags-menu-nueva-nota-oscuro-1280 | 11995 (`diff-tags-menu-nueva-nota-oscuro-1280.png`) |
| tags-nueva-nota-oscuro-1280 | 9041 (`diff-tags-nueva-nota-oscuro-1280.png`) |
| tags-nueva-nota-creada-oscuro-1280 | 8976 (`diff-tags-nueva-nota-creada-oscuro-1280.png`) |
| plantillas-oscuro-1280 | 814 (`diff-plantillas-oscuro-1280.png`) |
| plantillas-filtradas-oscuro-1280 | 4225 (`diff-plantillas-filtradas-oscuro-1280.png`) |
| plantillas-vacio-oscuro-1280 | 6722 (`diff-plantillas-vacio-oscuro-1280.png`) |
| plantilla-nombre-oscuro-1280 | 6722 (`diff-plantilla-nombre-oscuro-1280.png`) |
| plantilla-nombre-tomado-oscuro-1280 | 6722 (`diff-plantilla-nombre-tomado-oscuro-1280.png`) |
| plantilla-nombre-largo-oscuro-1280 | 6722 (`diff-plantilla-nombre-largo-oscuro-1280.png`) |
| plantilla-nombre-largo-inicio-oscuro-1280 | 6722 (`diff-plantilla-nombre-largo-inicio-oscuro-1280.png`) |
| plantilla-creada-oscuro-1280 | 46132 (`diff-plantilla-creada-oscuro-1280.png`) |
| nota-del-dia-oscuro-1280 | 0 |
| tags-menu-completo-oscuro-1280 | 0 |
| tags-alias-pregunta-oscuro-1280 | 0 |
| tags-alias-largos-oscuro-1280 | 0 |
| tags-alias-largos-inicio-oscuro-1280 | 0 |
| tags-alias-vista-previa-oscuro-1280 | 0 |
| tags-alias-aplicado-oscuro-1280 | 0 |
| tags-icono-invalido-oscuro-1280 | 0 |
| tags-pagina-oscuro-1280 | 7311 (`diff-tags-pagina-oscuro-1280.png`) |
| links-autocompletado-oscuro-1280 | 129893 (`diff-links-autocompletado-oscuro-1280.png`) |
| links-autocompletado-filtrado-oscuro-1280 | 53553 (`diff-links-autocompletado-filtrado-oscuro-1280.png`) |
| links-autocompletado-empate-oscuro-1280 | 77306 (`diff-links-autocompletado-empate-oscuro-1280.png`) |
| links-autocompletado-aceptado-oscuro-1280 | 14284 (`diff-links-autocompletado-aceptado-oscuro-1280.png`) |
| links-autocompletado-borde-oscuro-1280 | 173798 (`diff-links-autocompletado-borde-oscuro-1280.png`) |
| links-autocompletado-ruta-larga-oscuro-1280 | 176256 (`diff-links-autocompletado-ruta-larga-oscuro-1280.png`) |
| tabla-desalineada-oscuro-1280 | 29595 (`diff-tabla-desalineada-oscuro-1280.png`) |
| tabla-paleta-oscuro-1280 | 29595 (`diff-tabla-paleta-oscuro-1280.png`) |
| tabla-alineada-oscuro-1280 | 62168 (`diff-tabla-alineada-oscuro-1280.png`) |
| tabla-larga-alineada-oscuro-1280 | no hay captura de antes |
| focus-completar-carpetas-oscuro-1280 | no hay captura de antes |
| focus-completar-tags-oscuro-1280 | no hay captura de antes |
| focus-completar-espacios-oscuro-1280 | no hay captura de antes |
| focus-carpeta-con-espacios-oscuro-1280 | no hay captura de antes |
| focus-tags-oscuro-1280 | no hay captura de antes |
| focus-carpetas-oscuro-1280 | no hay captura de antes |
| focus-menu-oscuro-1280 | no hay captura de antes |
| focus-finder-oscuro-1280 | no hay captura de antes |
| focus-buscar-oscuro-1280 | no hay captura de antes |
| focus-largo-oscuro-1280 | no hay captura de antes |
| focus-largo-finder-oscuro-1280 | no hay captura de antes |
| focus-vacio-oscuro-1280 | no hay captura de antes |
| focus-finder-vacio-oscuro-1280 | no hay captura de antes |
| focus-desde-el-panel-oscuro-1280 | no hay captura de antes |
| focus-menu-salir-oscuro-1280 | no hay captura de antes |
| sin-focus-oscuro-1280 | no hay captura de antes |
| backlinks-oscuro-1280 | no hay captura de antes |
| backlinks-final-oscuro-1280 | no hay captura de antes |
| backlinks-plegado-oscuro-1280 | no hay captura de antes |
| backlinks-menu-borde-oscuro-1280 | no hay captura de antes |
| backlinks-abierto-oscuro-1280 | no hay captura de antes |
| sin-destino-oscuro-1280 | no hay captura de antes |
| sin-destino-filtrado-oscuro-1280 | no hay captura de antes |
| sin-destino-vacio-oscuro-1280 | no hay captura de antes |
| sin-destino-abierto-oscuro-1280 | no hay captura de antes |
| autocompletado-angosto-oscuro-1280 | no hay captura de antes |
| links-autocompletado-angosto-oscuro-1280 | no hay captura de antes |
| wrap-seleccion-oscuro-1280 | no hay captura de antes |
| sin-wrap-oscuro-1280 | no hay captura de antes |
| sin-wrap-palabra-oscuro-1280 | no hay captura de antes |
| editor-oscuro-1920 | 51383 (`diff-editor-oscuro-1920.png`) |
| editor-final-oscuro-1920 | 38152 (`diff-editor-final-oscuro-1920.png`) |
| arbol-oscuro-1920 | 51383 (`diff-arbol-oscuro-1920.png`) |
| menu-oscuro-1920 | 51383 (`diff-menu-oscuro-1920.png`) |
| menu-borde-oscuro-1920 | 43304 (`diff-menu-borde-oscuro-1920.png`) |
| split-oscuro-1920 | 228830 (`diff-split-oscuro-1920.png`) |
| linea-de-comandos-oscuro-1920 | 51383 (`diff-linea-de-comandos-oscuro-1920.png`) |
| finder-oscuro-1920 | 51383 (`diff-finder-oscuro-1920.png`) |
| finder-filtrado-oscuro-1920 | 71855 (`diff-finder-filtrado-oscuro-1920.png`) |
| finder-ruta-larga-oscuro-1920 | 212874 (`diff-finder-ruta-larga-oscuro-1920.png`) |
| finder-vacio-oscuro-1920 | 51383 (`diff-finder-vacio-oscuro-1920.png`) |
| finder-tags-oscuro-1920 | 73290 (`diff-finder-tags-oscuro-1920.png`) |
| finder-tags-inexistentes-oscuro-1920 | 231641 (`diff-finder-tags-inexistentes-oscuro-1920.png`) |
| finder-buzon-oscuro-1920 | 51383 (`diff-finder-buzon-oscuro-1920.png`) |
| buscar-oscuro-1920 | 235804 (`diff-buscar-oscuro-1920.png`) |
| buscar-vacio-oscuro-1920 | 51383 (`diff-buscar-vacio-oscuro-1920.png`) |
| buscar-escrito-largo-oscuro-1920 | 51383 (`diff-buscar-escrito-largo-oscuro-1920.png`) |
| buscar-cjk-oscuro-1920 | 51383 (`diff-buscar-cjk-oscuro-1920.png`) |
| paleta-oscuro-1920 | 51383 (`diff-paleta-oscuro-1920.png`) |
| paleta-final-oscuro-1920 | 51383 (`diff-paleta-final-oscuro-1920.png`) |
| paleta-save-oscuro-1920 | 51383 (`diff-paleta-save-oscuro-1920.png`) |
| paleta-focus-oscuro-1920 | 51383 (`diff-paleta-focus-oscuro-1920.png`) |
| paleta-workspace-oscuro-1920 | 51383 (`diff-paleta-workspace-oscuro-1920.png`) |
| paleta-pane-oscuro-1920 | 51383 (`diff-paleta-pane-oscuro-1920.png`) |
| paleta-panel-oscuro-1920 | 51383 (`diff-paleta-panel-oscuro-1920.png`) |
| paleta-editor-oscuro-1920 | 54537 (`diff-paleta-editor-oscuro-1920.png`) |
| marks-oscuro-1920 | 51542 (`diff-marks-oscuro-1920.png`) |
| marks-nota-larga-oscuro-1920 | 53213 (`diff-marks-nota-larga-oscuro-1920.png`) |
| marks-borde-derecho-oscuro-1920 | 142811 (`diff-marks-borde-derecho-oscuro-1920.png`) |
| listas-oscuro-1920 | 0 |
| listas-casilla-oscuro-1920 | 0 |
| formulas-oscuro-1920 | 0 |
| formulas-bloque-oscuro-1920 | 0 |
| formulas-larga-oscuro-1920 | 337497 (`diff-formulas-larga-oscuro-1920.png`) |
| autoguardado-guardado-oscuro-1920 | 0 |
| autoguardado-sin-guardar-oscuro-1920 | 0 |
| autoguardado-cambio-en-disco-oscuro-1920 | 0 |
| tags-oscuro-1920 | 0 |
| tags-abierto-oscuro-1920 | 0 |
| tags-buzon-oscuro-1920 | 0 |
| tags-menu-borde-oscuro-1920 | 0 |
| autocompletado-oscuro-1920 | 0 |
| autocompletado-alias-oscuro-1920 | 0 |
| autocompletado-elegir-oscuro-1920 | 0 |
| autocompletado-aceptado-oscuro-1920 | 0 |
| autocompletado-borde-oscuro-1920 | 303306 (`diff-autocompletado-borde-oscuro-1920.png`) |
| autocompletado-alias-largo-oscuro-1920 | 99996 (`diff-autocompletado-alias-largo-oscuro-1920.png`) |
| autocompletado-panel-bajo-oscuro-1920 | 7269 (`diff-autocompletado-panel-bajo-oscuro-1920.png`) |
| tags-menu-tag-oscuro-1920 | 16928 (`diff-tags-menu-tag-oscuro-1920.png`) |
| tags-renombrar-pregunta-oscuro-1920 | 17315 (`diff-tags-renombrar-pregunta-oscuro-1920.png`) |
| renombrar-tag-oscuro-1920 | 11398 (`diff-renombrar-tag-oscuro-1920.png`) |
| renombrar-tag-quitado-oscuro-1920 | 11398 (`diff-renombrar-tag-quitado-oscuro-1920.png`) |
| renombrar-tag-final-oscuro-1920 | 11398 (`diff-renombrar-tag-final-oscuro-1920.png`) |
| fusionar-tag-bloqueado-oscuro-1920 | 11398 (`diff-fusionar-tag-bloqueado-oscuro-1920.png`) |
| renombrar-tag-aplicado-oscuro-1920 | 17315 (`diff-renombrar-tag-aplicado-oscuro-1920.png`) |
| renombrar-tag-deshecho-oscuro-1920 | 17315 (`diff-renombrar-tag-deshecho-oscuro-1920.png`) |
| tags-menu-nueva-nota-oscuro-1920 | 17315 (`diff-tags-menu-nueva-nota-oscuro-1920.png`) |
| tags-nueva-nota-oscuro-1920 | 13297 (`diff-tags-nueva-nota-oscuro-1920.png`) |
| tags-nueva-nota-creada-oscuro-1920 | no hay captura de antes |
| plantillas-oscuro-1920 | no hay captura de antes |
| plantillas-filtradas-oscuro-1920 | no hay captura de antes |
| plantillas-vacio-oscuro-1920 | no hay captura de antes |
| plantilla-nombre-oscuro-1920 | no hay captura de antes |
| plantilla-nombre-tomado-oscuro-1920 | no hay captura de antes |
| plantilla-nombre-largo-oscuro-1920 | no hay captura de antes |
| plantilla-nombre-largo-inicio-oscuro-1920 | no hay captura de antes |
| plantilla-creada-oscuro-1920 | no hay captura de antes |
| nota-del-dia-oscuro-1920 | no hay captura de antes |
| tags-menu-completo-oscuro-1920 | no hay captura de antes |
| tags-alias-pregunta-oscuro-1920 | no hay captura de antes |
| tags-alias-largos-oscuro-1920 | no hay captura de antes |
| tags-alias-largos-inicio-oscuro-1920 | no hay captura de antes |
| tags-alias-vista-previa-oscuro-1920 | no hay captura de antes |
| tags-alias-aplicado-oscuro-1920 | no hay captura de antes |
| tags-icono-invalido-oscuro-1920 | no hay captura de antes |
| tags-pagina-oscuro-1920 | no hay captura de antes |
| links-autocompletado-oscuro-1920 | no hay captura de antes |
| links-autocompletado-filtrado-oscuro-1920 | no hay captura de antes |
| links-autocompletado-empate-oscuro-1920 | no hay captura de antes |
| links-autocompletado-aceptado-oscuro-1920 | no hay captura de antes |
| links-autocompletado-borde-oscuro-1920 | no hay captura de antes |
| links-autocompletado-ruta-larga-oscuro-1920 | no hay captura de antes |
| tabla-desalineada-oscuro-1920 | no hay captura de antes |
| tabla-paleta-oscuro-1920 | no hay captura de antes |
| tabla-alineada-oscuro-1920 | no hay captura de antes |
| tabla-larga-alineada-oscuro-1920 | no hay captura de antes |
| focus-completar-carpetas-oscuro-1920 | no hay captura de antes |
| focus-completar-tags-oscuro-1920 | no hay captura de antes |
| focus-completar-espacios-oscuro-1920 | no hay captura de antes |
| focus-carpeta-con-espacios-oscuro-1920 | no hay captura de antes |
| focus-tags-oscuro-1920 | no hay captura de antes |
| focus-carpetas-oscuro-1920 | no hay captura de antes |
| focus-menu-oscuro-1920 | no hay captura de antes |
| focus-finder-oscuro-1920 | no hay captura de antes |
| focus-buscar-oscuro-1920 | no hay captura de antes |
| focus-largo-oscuro-1920 | no hay captura de antes |
| focus-largo-finder-oscuro-1920 | no hay captura de antes |
| focus-vacio-oscuro-1920 | no hay captura de antes |
| focus-finder-vacio-oscuro-1920 | no hay captura de antes |
| focus-desde-el-panel-oscuro-1920 | no hay captura de antes |
| focus-menu-salir-oscuro-1920 | no hay captura de antes |
| sin-focus-oscuro-1920 | no hay captura de antes |
| backlinks-oscuro-1920 | no hay captura de antes |
| backlinks-final-oscuro-1920 | no hay captura de antes |
| backlinks-plegado-oscuro-1920 | no hay captura de antes |
| backlinks-menu-borde-oscuro-1920 | no hay captura de antes |
| backlinks-abierto-oscuro-1920 | no hay captura de antes |
| sin-destino-oscuro-1920 | no hay captura de antes |
| sin-destino-filtrado-oscuro-1920 | no hay captura de antes |
| sin-destino-vacio-oscuro-1920 | no hay captura de antes |
| sin-destino-abierto-oscuro-1920 | no hay captura de antes |
| autocompletado-angosto-oscuro-1920 | no hay captura de antes |
| links-autocompletado-angosto-oscuro-1920 | no hay captura de antes |
| wrap-seleccion-oscuro-1920 | no hay captura de antes |
| sin-wrap-oscuro-1920 | no hay captura de antes |
| sin-wrap-palabra-oscuro-1920 | no hay captura de antes |
| saltos-al-abrir-claro-1280 | no hay captura de antes |
| saltos-ctrl-o-claro-1280 | no hay captura de antes |
| saltos-mark-b-claro-1280 | no hay captura de antes |
| saltos-al-abrir-claro-1920 | no hay captura de antes |
| saltos-ctrl-o-claro-1920 | no hay captura de antes |
| saltos-mark-b-claro-1920 | no hay captura de antes |
| saltos-al-abrir-oscuro-1280 | no hay captura de antes |
| saltos-ctrl-o-oscuro-1280 | no hay captura de antes |
| saltos-mark-b-oscuro-1280 | no hay captura de antes |
| saltos-al-abrir-oscuro-1920 | no hay captura de antes |
| saltos-ctrl-o-oscuro-1920 | no hay captura de antes |
| saltos-mark-b-oscuro-1920 | no hay captura de antes |
