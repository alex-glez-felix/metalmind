# nota-vim

Motor vim de `nota`: máquina de estados pura sobre `nota-buffer`, sin GUI ni I/O.

- `Key` → `Keymap` (tabla tipada de contexto + teclas + acción) → `Action`. Cada acción tiene nombre (`Action::name`, `Action::from_name`).
- `Vim::feed(key, &mut buffer, &mut selections)` ejecuta; `Vim::execute(action, …)` permite invocar acciones por nombre desde la paleta o `:`.
- `Mode`: Normal, Insert, operador pendiente y línea de comandos (`:`).
- La línea de comandos reconoce `:w`, `:q`, `:q!` y `:wq` (con abreviaturas, como vim) pero no los ejecuta: `feed` devuelve `KeyOutcome::Command(Command)` y quien aloja el editor invoca la acción con nombre (`editor::save`, `editor::quit`, `editor::force_quit`, `editor::save_and_quit`), porque guardar y cerrar hacen I/O.
- Referencia: `nvim --clean` 0.12.6 con `noautoindent`. `tests/vim.rs` corre cada caso en el motor y en nvim; el oráculo falla si el `nvim` del PATH es otra versión. Le manda las teclas a un nvim `--embed` con `nvim_input`, como la TUI, para que cuenten como tecleadas: un `<C-c>` así interrumpe y no repite el insert con count (`3ihola<C-c>`), cosa que con `nvim_feedkeys` no pasa.

Fuera de alcance por ahora: Visual, `.`, macros, registros con nombre, `f/t`, autoindent.
