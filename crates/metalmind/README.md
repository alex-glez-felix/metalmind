# metalmind

El binario: abre la ventana o, con un subcomando, trabaja sobre el vault sin ella. Toda la lógica vive en los otros crates; aquí se decide qué abrir y se conecta lo demás.

```sh
metalmind ~/Notas           # esa carpeta es el vault
metalmind ~/Notas/idea.md   # el vault de su carpeta, con la nota abierta
metalmind                   # METALMIND_VAULT, el último abierto o `vault:` de config.yaml
metalmind add "texto"       # los subcomandos: add, search, tags y open
```

- `target.rs`: qué vault y qué archivo abrir según los argumentos.
- `gui.rs`: la ventana, lo único del binario que toca GPUI. Preparar (config, vault y documento) va aparte de abrirla, para que `metalmind open --json` anuncie la nota solo cuando ya no puede fallar al leerla.
- `cli.rs` y `commands/`: los subcomandos, que abren el vault desde el núcleo, sin ventana ni GPUI; `open` solo resuelve la nota, y la ventana la abre `main`. La referencia, con su `--json`, está en [`docs/cli.md`](../../docs/cli.md).

Tests: `tests/cli.rs` y `tests/commands.rs` corren los subcomandos contra el binario de verdad y sin pantalla. `tests/gui_smoke.rs` lanza la app y le teclea, y `tests/capturas.rs` saca las capturas de `scripts/capturas.sh`: ver [`docs/testing.md`](../../docs/testing.md).
