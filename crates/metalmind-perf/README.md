# metalmind-perf

Soporte de rendimiento, solo para benchmarks y tests: ningún crate de la app depende de él para compilar en release. Los números objetivo y cómo se miden están en [`docs/performance.md`](../../docs/performance.md).

- **Datos sintéticos y deterministas**, nada de notas reales: `note`, `huge_line` y las entradas patológicas (`backtick_runs`, `hash_runs`, `unclosed_wikilinks`…) para el texto; `vault_paths`, `write_vault`, `tagged_vault` y `linked_vault` para los vaults; `math_note` y `formulas_note` para las fórmulas.
- `Session`: una nota abierta en el motor vim, para medir teclas como las recibe el editor (`Session::feed`).
- **Pruebas de complejidad**: `assert_linear` y `assert_flat` cuentan las instrucciones de una operación con N y con 16N (`FACTOR`), cada tamaño bajo callgrind en un proceso aparte, y fallan si crece más de `MAX_LINEAR_GROWTH` o `MAX_FLAT_GROWTH`.
- `benches/instrucciones.rs`: los benchmarks del núcleo, contados en instrucciones con `gungraun`. El CI los corre en las PR que cambian código o configuración medidos, contra la base de la PR en el mismo runner, y `MAX_INSTRUCTION_REGRESSION` es el límite de subida.

Necesita `valgrind` para contar instrucciones y `clang` para compilar `valgrind-requests`: ver [`docs/performance.md`](../../docs/performance.md).
