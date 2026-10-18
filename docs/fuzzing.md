# Fuzzing

Los tests de tabla contra nvim cubren lo que se espera del motor vim. El
fuzzing busca lo que nadie pensó: pánicos, offsets que parten un carácter y
estados del motor que no deberían existir.

## Qué hay

El crate `fuzz/` usa [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
(libFuzzer) con entradas estructuradas de
[`arbitrary`](https://docs.rs/arbitrary). Vive fuera del workspace: necesita
nightly y sanitizers, y así no entra a `cargo clippy --workspace` ni al CI de
las PRs. El resto del repo sigue en stable.

| Target | Qué le llega | Qué se exige |
|---|---|---|
| `buffer_edits` | Lotes de ediciones (válidos o no), commits, undo/redo, `g-`/`g+`, grupos de undo y anclas sobre un texto arbitrario. | Nunca hay pánico. Un lote inválido da error y no toca el texto; uno válido deja lo mismo que aplicarlo sobre un `String`. Cada estado del historial muestra el texto que tenía al crearse, se llegue como se llegue. Deshacer todo da el texto inicial y rehacer todo, el del último estado. Las anclas caen en frontera de char. |
| `vim_keys` | Teclas, pegados y clics al motor vim sobre un texto arbitrario, con uno a cuatro cursores, más comandos de varias teclas que no saldrían tecla por tecla: `v`/`V`, `"{registro}`, `q{registro}`/`q`, `@{registro}`/`@@`, `m{mark}`, `'{mark}`/`` `{mark} ``, `/`/`?` con un patrón tecleado, `n`/`N`, y cambios del portapapeles hechos por otra aplicación. | Nunca hay pánico ni error de buffer. Las selecciones caen dentro del texto y en frontera de char. En Normal y con un operador pendiente solo hay cursores, nunca sobre el fin de una línea con texto. Fuera de Insert no queda una transacción abierta; en Insert sí hay una. La línea de comandos existe solo en su modo. |
| `vim_search_subst` | Patrones para `/` y `?`, `:s` con delimitador, reemplazo y flags arbitrarios, líneas de `:` sueltas, y `n`, `N`, `*`, `#`, `&`, `u`, `ctrl-r`. | Lo mismo que `vim_keys`, y la línea de comandos siempre se cierra: un patrón inválido es un error, no un crash. |

Las piezas comunes (teclas y comandos arbitrarios, cursores válidos, el
portapapeles e invariantes del motor) están en `fuzz/src/lib.rs`.

Dos límites evitan que el fuzzer persiga lentitud esperada en vez de bugs:
`vim_keys` descarta los dígitos de un count a partir del tercero seguido
(`999999999ix` tarda lo mismo en vim), y los dos targets de vim se detienen si
el texto pasa de 64 KiB.

## Correrlo en local

```sh
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked

cargo +nightly fuzz list
cargo +nightly fuzz run vim_keys -- -max_total_time=600 -timeout=10 -rss_limit_mb=2048
```

`+nightly` pasa por encima del `rust-toolchain.toml`. Con `-fork=N` corre en
N procesos; si se agrega `-ignore_crashes=1`, sigue tras un crash y junta
varios en una sola corrida. El corpus queda en `fuzz/corpus/<target>/` y los
crashes en `fuzz/artifacts/<target>/`; los dos están en el `.gitignore`.

## Cuando aparece un crash

1. **Reproducirlo y minimizarlo.**

   ```sh
   cargo +nightly fuzz run vim_keys fuzz/artifacts/vim_keys/crash-…
   cargo +nightly fuzz tmin vim_keys fuzz/artifacts/vim_keys/crash-…
   cargo +nightly fuzz fmt vim_keys fuzz/artifacts/vim_keys/minimized-from-…
   ```

   `fmt` imprime la entrada con `Debug`: el texto inicial, los cursores y las
   teclas.
2. **Decidir de quién es la culpa.** Si el fuzzer exige algo que la UI no
   hace (un clic donde el host no deja el cursor, por ejemplo), se corrige el
   fuzzer. Si no, el bug es del crate y se arregla ahí, en la causa: el fuzzer
   no se toca para esquivarlo.
3. **Convertirlo en un test normal** en el crate que se arregló, con un commit
   por arreglo:
   - `nota-buffer`: un `#[test]` en `crates/nota-buffer/tests/buffer.rs`.
   - `nota-vim`: si nvim puede hacer lo mismo, un caso de la tabla del
     oráculo en `crates/nota-vim/tests/vim.rs` (`"tex|to"`, teclas en
     notación de vim, resultado esperado); si depende de algo que nvim no
     tiene, como varios cursores, un caso en `multiple_cursors` o un test
     propio. Las teclas de `fmt` se escriben con la notación de
     `tests/support/mod.rs` (`<Esc>`, `<CR>`, `<C-r>`).
4. Volver a correr el target con el crash como entrada y confirmar que pasa.

## En CI

`.github/workflows/fuzz.yml` corre cada target cinco minutos a diario y con
`workflow_dispatch` (se puede pedir otra duración). El corpus se guarda en el
caché de Actions entre corridas, también cuando hay un crash, y se minimiza
con `cmin` cuando no lo hay. Si un target falla, el crash se sube como artifact
`crash-<target>`: se descarga y se sigue el paso 1 con ese archivo.
