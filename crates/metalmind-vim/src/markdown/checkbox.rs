//! `editor::toggle_checkbox`: `- [ ]` ↔ `- [x]`.

use metalmind_buffer::{Buffer, Edit};

use super::{LazyStyles, ListItem};

/// Las ediciones que marcan o desmarcan el checkbox de cada ítem de `lines`.
/// Un ítem sin checkbox recibe uno vacío; las líneas que no son ítems o
/// están en un bloque de código o en el frontmatter no cambian.
pub(crate) fn toggle_checkboxes(
    buffer: &Buffer,
    lines: impl IntoIterator<Item = usize>,
    styles: &LazyStyles,
) -> Vec<Edit> {
    lines
        .into_iter()
        .filter_map(|line| {
            let text = buffer.line_text(line);
            let item = ListItem::parse(&text)?;
            if styles.outline(buffer).is_literal(line) {
                return None;
            }
            let start = buffer.line_start(line);
            Some(match item.checkbox.clone() {
                Some(inside) => {
                    let mark = if item.is_checked() { " " } else { "x" };
                    Edit::replace(start + inside.start..start + inside.end, mark)
                }
                None => Edit::insert(start + item.content, "[ ] "),
            })
        })
        .collect()
}
