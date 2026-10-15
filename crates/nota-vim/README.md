# nota-vim

Motor vim de `nota`: máquina de estados pura sobre `nota-buffer`, sin GUI ni I/O.

- `Key` → `Keymap` (tabla tipada de contexto + teclas + acción) → `Action`. Cada acción tiene nombre (`Action::name`, `Action::from_name`).
- `Vim::feed(key, &mut buffer, &mut selections)` ejecuta; `Vim::execute(action, …)` permite invocar acciones por nombre desde la paleta o `:`.
- `Mode`: Normal, Insert, operador pendiente.
- Referencia: `nvim --clean` 0.12.6 con `noautoindent`. `tests/vim.rs` corre cada caso en el motor y en nvim; el oráculo falla si el `nvim` del PATH es otra versión.

Fuera de alcance por ahora: Visual, `.`, macros, registros con nombre, `f/t`, autoindent.
