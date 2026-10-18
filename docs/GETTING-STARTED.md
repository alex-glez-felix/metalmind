# Primeros pasos

Resumen accionable de `docs/research/`. Si algo aquí contradice a un documento de investigación, gana el documento; este archivo solo ordena.

## Decisiones tomadas

| Tema | Decisión | Por qué | Detalle |
|---|---|---|---|
| GUI | **GPUI** vía `gpui-kit =0.7.3` sin features por defecto (solo `gpui-pre`, sin gpui-component), con **editor propio** | Único stack probado en un editor de alto rendimiento en Linux (Zed). El spike confirmó 240 fps, teclas muertas, IME y portapapeles en la máquina de pruebas; el editor de gpui-component no escala (108 s para abrir 50 MB) | 02, 04 |
| Plan B | **iced 0.14 + cosmic-text** | La IA lo conoce mejor y crates.io no está desfasado; el editor habría que hacerlo a mano | 02 |
| Buffer | `ropey` + undo/multi-cursor propios | Estándar, estable | 03 §1 |
| Markdown | `tree-sitter-md` (editor, incremental) + `pulldown-cmark` (índice/export) | Cada uno para lo que sirve | 03 §2 |
| Vim | Motor **propio**, puro, sin GUI, con el modelo de keymap+contextos+acciones de Zed | Así vim aplica igual al árbol, la paleta y `:` | 03 §3 |
| Vault | `ignore` + `notify` + índice en memoria; `nucleo` para fuzzy | Los `.md` son la verdad; caché solo para arranque | 03 §4 |

## Pendiente de decidir (del autor)

1. ~~Licencia~~ **Decidido 2026-10-15: GPL-3.0-or-later** (`LICENSE`). Contexto: el motor vim es propio, pero los agentes van a querer copiar o adaptar trozos de Zed. Verificado 2026-10-15: `crates/vim` y `crates/editor` de Zed son **GPL-3.0-or-later**; `gpui` es Apache-2.0. Si `metalmind` es MIT/Apache, Zed se lee pero no se copia. Si es GPL, se puede adaptar código de Zed. Bloom es AGPL: solo ideas.
2. ~~Nombre~~ **Decidido 2026-10-18: Metalmind**, por las mentes de metal de la Feruquimia en *Mistborn*.
3. **¿Embeber Neovim o emular vim?** Emular es lo recomendado (control de UX total). Embeber (como kimun) da fidelidad gratis pero ata la UX a nvim.

## Fase 0 — Spike (hecho, 2026-10-17)

Objetivo: confirmar GPUI antes de escribir el núcleo. Si falla, se cambia a iced sin haber perdido nada, porque el núcleo no depende de la GUI.

El spike ya está armado en [`spikes/gpui/`](../spikes/gpui/README.md): un proyecto aparte, fuera del workspace, con la vista propia (`metalmind-buffer` + `metalmind-vim`) y el editor de gpui-kit para comparar.

1. Preparar la máquina de pruebas y correr `./spikes/gpui/run.sh`, como dice su README.
2. Seguir la checklist del README en Hyprland y, si se puede, en GNOME/KDE en VM. Cubre:
   - archivo de 50 MB, scroll fluido, con tiempos de frame medidos
   - IME (Fcitx5) y dead keys (`´` + `a` = `á` — importa para español) traducidas a `metalmind-vim`
   - portapapeles normal y primario (selección con el mouse)
3. Anotar resultados en `docs/research/04-spike-gpui.md`.

Resultado: GPUI sí, editor propio. Detalle en [04](research/04-spike-gpui.md).

## Correr `metalmind`

Las librerías de sistema son las mismas del spike (una sola vez):

```sh
sudo pacman -S --needed base-devel fontconfig freetype2 libxkbcommon libxkbcommon-x11 wayland libxcb vulkan-icd-loader
```

Desde la raíz del repo:

```sh
git pull
cargo build --release
./target/release/metalmind /tmp/prueba.md          # si no existe, abre vacío y se crea con :w
RUST_LOG=metalmind_ui=info,gpui_wgpu=info ./target/release/metalmind /tmp/prueba.md   # GPU elegida y tiempo al primer frame
cargo install --path crates/metalmind               # opcional: deja `metalmind` en ~/.cargo/bin
```

Sin ventana, la CLI del vault ([cli.md](cli.md)): `METALMIND_VAULT=/tmp/v ./target/release/metalmind add "idea"`, `metalmind search`, `metalmind open`, con `--json`.

Dentro: vim normal/insert (lo de `metalmind-vim`; Visual con `v`, `V` y `ctrl-v` opera y se pinta como en vim), `:w` guarda, `:q` cierra el panel (se niega si se perderían cambios: `[+]` en la barra y en el título), `:wq`, `:q!`. `:e otra.md` abre otro archivo en el panel (relativo a la raíz del vault) y `:e!` descarta los cambios. `ctrl-w v`/`ctrl-w s` dividen, `ctrl-w h/j/k/l/w` mueven el foco, `ctrl-w q` cierra el panel y `ctrl-w o` deja solo este. Para el smoke sin GPU: `scripts/smoke-x11.sh` (necesita `xvfb-run`, `xdotool` y lavapipe). `ctrl-shift-c`/`ctrl-shift-v` copian y pegan con el portapapeles; seleccionar con el ratón copia a la primaria y el clic medio la pega. La fuente es la primera instalada de la lista en `crates/metalmind-ui/src/theme.rs` (por ejemplo, JetBrainsMono Nerd Font).

### Compilar rápido

Las dependencias van con `opt-level = 2` en debug (`Cargo.toml`): el primer build tarda más, pero la app en debug corre fluida. Lo demás es opcional y vive en tu máquina, no en el repo, para que `cargo build` funcione en cualquier lado:

- **mold** acorta el enlace de cada rebuild (en un escritorio de 16 hilos, de 1.5 s a 0.9 s). Con `clang` y `mold` instalados (`sudo pacman -S --needed clang mold` o `sudo apt-get install -y clang mold`), agrega a `~/.cargo/config.toml`:

  ```toml
  [target.x86_64-unknown-linux-gnu]
  linker = "clang"
  rustflags = ["-C", "link-arg=-fuse-ld=mold"]
  ```

- **sccache** cachea las dependencias entre checkouts y worktrees (útil con varios agentes, cada uno con el suyo): `export RUSTC_WRAPPER=sccache`. No acelera los crates propios en debug, que ya son incrementales.

## Hitos después del spike

M0 ventana y texto → M1 vim normal/insert → M2 vault y finder → M3 wikilinks y backlinks → M4 live preview → M5 embeds y callouts → M6 búsqueda, pulido, primer release. Detalle en 03 §8.

## Código para leer

- **Velotype** (GPUI, Apache-2.0): render rico sobre GPUI; se puede reutilizar.
- **Zed `crates/vim`**: la implementación modal más madura (revisar licencia).
- **Markdown Oxide**: semántica de vaults Obsidian (links, backlinks, tags).
- **Bloom** (iced, AGPL): vim en toda la app; solo ideas.

## Diferenciador

Ya hay docenas de "Obsidian en Rust" vibe-codeados con 0★. Lo que no hay: **latencia medida, vaults de 10k+ notas y vim fiel en toda la app**. Medir desde M0 (benchmarks en CI) es la ventaja, no la lista de features.
