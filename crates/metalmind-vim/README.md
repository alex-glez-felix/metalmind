# metalmind-vim

Motor vim de Metalmind: máquina de estados pura sobre `metalmind-buffer`, sin GUI ni I/O. Qué hace cada tecla y en qué se aparta de vim está en [`docs/vim.md`](../../docs/vim.md); aquí, cómo se usa desde el código.

```rust
let mut vim = Vim::new();
let mut target = Target {
    buffer: &mut buffer,
    selections: &mut selections,
    doc: &mut doc,
    pane: &mut pane,
    file,
    clipboard: None,
};
match vim.feed(Key::Char('d'), &mut target)? {
    KeyOutcome::Dispatch(invocation) => { /* `:w`, `ctrl-w v`: lo ejecuta quien aloja el motor */ }
    KeyOutcome::SwitchFile(landing) => { /* cambia el buffer del panel y llama a `Vim::land` */ }
    _ => {}
}
```

Una macro se pausa en esos dos resultados: al terminar con ellos, quien aloja el motor llama a `Vim::resume_macro` para seguirla, o a `Vim::abort_macro` si el comando falló o se llevó las teclas.

- **Teclas y acciones.** `Key` → `Keymap` (tabla tipada de contexto + teclas + acción) → `Action`. Cada acción tiene nombre (`Action::name`, `Action::from_name`). `Vim::feed` recibe una tecla y `Vim::execute` una acción, como las que invoca la paleta. `Key` se escribe como los atajos de GPUI (`ctrl-w`, `space`).
- **`Target`, lo que se le presta.** `Vim` guarda lo que es de todo el editor: el modo y lo pendiente, los registros, las macros, las marks `A`–`Z`, los historiales de `:` y `/`. Lo demás llega en cada tecla, prestado por referencia: el `Buffer` y las `Selections` del panel, el `DocState` del documento, el `PaneState` del panel (su `Viewport` y su `JumpList`), el `FileId` y, si hay, el `Clipboard` que atiende `"+` y `"*`. Un `Target` armado con las piezas de otro panel o de otro documento es igual de válido.
- **Lo editado lo sigue el motor solo**: el `DocState` sabe hasta qué `Splice` del buffer siguieron la jumplist y las marks `A`–`Z` (`Buffer::splice_count`). Quien saque los reemplazos con `Buffer::take_splices` antes de la próxima llamada al motor se los quita; en la app, el workspace los saca justo después de él.
- **Lo que no es del motor.** Solo conoce sus acciones de edición. Quien lo aloja le agrega al `Keymap` atajos que resuelven a una acción ajena (`Bound::Host(ActionName)`) y comandos de `:` (`HostCommand`). El motor los reconoce con las reglas de vim, pero no los ejecuta: `feed` devuelve `KeyOutcome::Dispatch(Invocation)`. `Keymap::vim()` trae solo las teclas de edición; las de la app las declara `metalmind-keymap`.
- **La ventana.** El motor no conoce la pantalla: quien lo aloja le da el alto a su `Viewport` en cada layout (`Viewport::resize`) y pinta desde `Viewport::top`, y el motor lo mueve tras cada tecla. El `DocState` de un documento que se pinta (`DocState::painted`) guarda el estilo y lo que mide cada línea. Ver [`docs/vim.md`](../../docs/vim.md#scroll-y-la-ventana).
- **Selecciones en Visual.** Cada `Selection` va de `anchor` (el extremo fijo) a `head` (el cursor), los dos incluidos. Con `VisualKind::Line` se pintan las líneas completas, y con `VisualKind::Block` lo dice `VisualBlock::new`. Ver [`docs/vim.md`](../../docs/vim.md#visual).

Tests: cada caso de `tests/vim.rs`, `tests/visual.rs`, `tests/visual_block.rs` y `tests/scroll.rs` corre en el motor y en `nvim`, que es el oráculo ([`docs/vim.md`](../../docs/vim.md#cómo-se-comprueba)).
