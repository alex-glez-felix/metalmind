# Plantillas y nota del día

Una plantilla es una nota markdown en la carpeta `plantillas/` del vault (`templates_folder` en [config](config.md)), con subcarpetas si se quiere. Su nombre es su ruta dentro de la carpeta sin `.md`: `entrevista`, `trabajo/semanal`. Las reglas de fondo están en [06](research/06-decisiones-de-fondo.md) §11.

## Variables

| Variable | Da |
|---|---|
| `{{title}}` | El nombre de la nota, sin carpetas ni `.md` |
| `{{date}}` | La fecha local, `2026-10-26` |
| `{{time}}` | La hora local, `15:30` |
| `{{date:FORMATO}}`, `{{time:FORMATO}}` | Otra forma: `{{date:dddd D [de] MMMM}}` da `lunes 26 de octubre` |

Los códigos de formato son los más comunes de moment.js (`YYYY MM DD dddd HH mm`…; la lista completa en 06 §11). Una variable mal escrita se deja tal cual, la nota se crea igual y la barra de estado dice cuál fue.

## Crear una nota desde una plantilla

- **`space n`**, la paleta (`notes: new from template`) o **`:template`**: un picker con las plantillas; a la derecha de cada una, los tags cuya página la pide. Al elegir, se pregunta el nombre de la nota: relativo a la raíz del vault, con `/` para carpetas, que se crean si faltan. La fila muestra la ruta que se va a crear; si el nombre está tomado o no vale, lo dice y la pregunta sigue abierta. `enter` la crea y la abre; `ctrl-v` y `ctrl-s`, en un split.
- **`:template entrevista Backend senior`** hace lo mismo sin preguntar: la primera palabra es la plantilla y el resto, el nombre. Con solo la plantilla, pregunta el nombre.
- **Desde un tag**: en la vista de tags del panel lateral, `a` o "Nueva nota con este tag" en su menú. La nota nace con el tag y con la plantilla que pida su página (`template: entrevista` en `tags/entrevista.md`, [05](research/05-tags.md) §7).
- **Desde la terminal**: `metalmind add --template entrevista --title "Backend senior" "Sabe Rust"` crea `backend-senior.md` con el texto donde iría el cursor, y `metalmind add -t entrevista …` usa la plantilla que pida la página del tag ([cli](cli.md)).

Una plantilla elegida a mano trae los tags de su propio frontmatter y, si la pide la página de **un solo** tag, también ese tag. Si la piden varias, no se pone ninguno: elegir uno adivinaría.

El cursor queda bajo el primer heading con la sección vacía, sin contar un título cuyo contenido son solo subsecciones; en la plantilla de abajo, bajo `## Resumen`. Si no hay un heading así, en la primera línea vacía, y si tampoco, al final.

## Nota del día

**`space d`**, la paleta (`notes: open daily`) o **`:today`** abren la nota de hoy y la crean si no existe, con la plantilla de `daily.template`. Está apagada por defecto ([06](research/06-decisiones-de-fondo.md) §9 y §11): se prende en `config.yaml`.

```yaml
daily:
  enabled: true
  folder: diario          # la carpeta, que se crea si falta
  format: YYYY-MM-DD      # un / hace subcarpetas: YYYY/MM/DD
  template: diario
```

## Ejemplo: una entrevista

`plantillas/entrevista.md`, para probar todo lo de arriba. Con `tags/entrevista.md` que diga `template: entrevista`, `a` sobre `#entrevista` en el panel la usa sola.

```markdown
---
tags: [entrevista]
puesto:
---
# {{title}}

{{date:dddd D [de] MMMM [de] YYYY}}, {{time}}

## Resumen

## Preguntas

- ¿Qué proyecto reciente te enseñó algo que no esperabas?
- Cuéntame de una decisión técnica que hoy tomarías distinto.
- ¿Cómo revisas el código de alguien con más experiencia que tú?
- ¿Qué preguntas tienes para nosotros?

## Evaluación

| Criterio | 1–5 | Evidencia |
|---|---|---|
| Comunicación | | |
| Diseño técnico | | |
| Código | | |
| Colaboración | | |

## Notas en vivo

```

`:template entrevista entrevistas/Backend senior` crea `entrevistas/Backend senior.md` con `# Backend senior`, la fecha de hoy en palabras y el cursor bajo `## Resumen`.
