# Primeros pasos

Resumen accionable de `docs/research/`. Si algo aquí contradice a un documento de investigación, gana el documento; este archivo solo ordena.

## Decisiones tomadas (provisionales hasta el spike)

| Tema | Decisión | Por qué | Detalle |
|---|---|---|---|
| GUI | **GPUI** vía `gpui-kit` (ex gpui-component, Longbridge), versión fijada exacta | Único stack probado en un editor de alto rendimiento en Linux (Zed). Trae editor de 200K líneas, render de markdown, listas virtuales | 02 |
| Plan B | **iced 0.14 + cosmic-text** | La IA lo conoce mejor y crates.io no está desfasado; el editor habría que hacerlo a mano | 02 |
| Buffer | `ropey` + undo/multi-cursor propios | Estándar, estable | 03 §1 |
| Markdown | `tree-sitter-md` (editor, incremental) + `pulldown-cmark` (índice/export) | Cada uno para lo que sirve | 03 §2 |
| Vim | Motor **propio**, puro, sin GUI, con el modelo de keymap+contextos+acciones de Zed | Así vim aplica igual al árbol, la paleta y `:` | 03 §3 |
| Vault | `ignore` + `notify` + índice en memoria; `nucleo` para fuzzy | Los `.md` son la verdad; caché solo para arranque | 03 §4 |

## Pendiente de decidir (tuyo, Alex)

1. **Licencia.** El motor vim es propio, pero los agentes van a querer copiar o adaptar trozos de Zed. Verificado 2026-10-15: `crates/vim` y `crates/editor` de Zed son **GPL-3.0-or-later**; `gpui` es Apache-2.0. Si `nota` es MIT/Apache, Zed se lee pero no se copia. Si es GPL, se puede adaptar código de Zed. Bloom es AGPL: solo ideas.
2. **Nombre.** `nota` es provisional.
3. **¿Embeber Neovim o emular vim?** Emular es lo recomendado (control de UX total). Embeber (como kimun) da fidelidad gratis pero ata la UX a nvim.

## Fase 0 — Spike (1–2 semanas, en pc-oma)

Objetivo: confirmar GPUI antes de escribir el núcleo. Si falla, se cambia a iced sin haber perdido nada, porque el núcleo no depende de la GUI.

1. Instalar Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Clonar `gpui-kit` y correr sus ejemplos de editor y markdown.
3. App mínima: ventana + editor de gpui-kit abriendo un `.md`.
4. Probar en Hyprland (tu Omarchy) y, si se puede, en GNOME/KDE en VM:
   - archivo de 50 MB, scroll fluido
   - IME (Fcitx5) y dead keys (`´` + `a` = `á` — importa para español)
   - portapapeles normal y primario (selección con el mouse)
5. Anotar resultados en `docs/research/04-spike-gpui.md`.

## Hitos después del spike

M0 ventana y texto → M1 vim normal/insert → M2 vault y finder → M3 wikilinks y backlinks → M4 live preview → M5 embeds y callouts → M6 búsqueda, pulido, primer release. Detalle en 03 §8.

## Código para leer

- **Velotype** (GPUI, Apache-2.0): render rico sobre GPUI; se puede reutilizar.
- **Zed `crates/vim`**: la implementación modal más madura (revisar licencia).
- **Markdown Oxide**: semántica de vaults Obsidian (links, backlinks, tags).
- **Bloom** (iced, AGPL): vim en toda la app; solo ideas.

## Diferenciador

Ya hay docenas de "Obsidian en Rust" vibe-codeados con 0★. Lo que no hay: **latencia medida, vaults de 10k+ notas y vim fiel en toda la app**. Medir desde M0 (benchmarks en CI) es la ventaja, no la lista de features.
