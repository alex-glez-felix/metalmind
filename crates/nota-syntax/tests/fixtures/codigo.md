Antes del código [[visible]] #visible

```markdown
[[dentro-del-bloque]] #dentro
```

~~~
[[dentro-de-tildes]] #tildes
```
sigue dentro
~~~

````
```
[[cerca-más-corta-no-cierra]]
````

Inline: `[[en-codigo]]` y ``doble `[[con-backtick]]` `` y `#tag-en-codigo`.
Sin cierre: ` suelto [[sí-cuenta]].

Código inline que cruza líneas: `empieza
[[sigue-en-codigo]]` y termina. Después [[afuera]].

`no cruza

[[párrafo-nuevo]]`

Comentario %%[[comentado]] #comentado%% y [[después-del-comentario]].
%%
Bloque de comentario
```
[[todo-comentado]]
%%
- Lista
  ```
  [[en-lista]]
  ```
> ```
> [[en-cita]]
> ```

[[al-final]] %% comentario sin cierre
[[comentado-hasta-el-final]] #nada
