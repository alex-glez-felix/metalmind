//! Tokens de tema: todo color y tamaño de la UI sale de aquí.

use gpui_kit::{App, FontWeight, Global, Hsla, Pixels, SharedString, Styled, px, relative, rgb};
use metalmind_config as config;

/// Familias monoespaciadas, en orden de preferencia. GPUI no resuelve alias
/// de fontconfig como `monospace` y entra en pánico con una familia que no
/// existe, así que se elige una instalada antes de pintar.
const MONO_FONTS: &[&str] = &[
    "JetBrainsMono Nerd Font",
    "JetBrains Mono",
    "Lilex",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
    "Liberation Mono",
    "Ubuntu Mono",
];

#[derive(Debug, thiserror::Error)]
#[error("no hay ninguna fuente monoespaciada instalada; se buscó: {}", MONO_FONTS.join(", "))]
pub struct NoMonoFont;

/// Cómo se ve el markdown en el editor (`syntax` y `headings` del tema).
#[derive(Clone, Debug)]
pub struct Markdown {
    pub heading: Hsla,
    pub bold: Hsla,
    pub italic: Hsla,
    pub link: Hsla,
    pub link_unresolved: Hsla,
    pub embed: Hsla,
    pub tag: Hsla,
    pub code: Hsla,
    pub code_background: Hsla,
    pub code_block_background: Hsla,
    pub quote: Hsla,
    pub list_marker: Hsla,
    pub checkbox: Hsla,
    pub task_done: Hsla,
    pub highlight: Hsla,
    pub comment: Hsla,
    pub frontmatter: Hsla,
    pub frontmatter_key: Hsla,
    pub block_id: Hsla,
    pub fence_info: Hsla,
    pub code_keyword: Hsla,
    pub code_string: Hsla,
    pub code_number: Hsla,
    pub code_comment: Hsla,
    pub markup: Hsla,
    /// Cada nivel, del `#` al `######`: veces la fuente del editor y peso.
    pub headings: [(f32, FontWeight); 6],
    /// El grosor del tachado.
    pub strike_thickness: Pixels,
}

impl Markdown {
    fn new(
        syntax: &config::SyntaxColors,
        headings: &config::Headings,
        stroke: &config::Stroke,
    ) -> Self {
        Self {
            heading: hsla(syntax.heading),
            bold: hsla(syntax.bold),
            italic: hsla(syntax.italic),
            link: hsla(syntax.link),
            link_unresolved: hsla(syntax.link_unresolved),
            embed: hsla(syntax.embed),
            tag: hsla(syntax.tag),
            code: hsla(syntax.code),
            code_background: hsla(syntax.code_background),
            code_block_background: hsla(syntax.code_block_background),
            quote: hsla(syntax.quote),
            list_marker: hsla(syntax.list_marker),
            checkbox: hsla(syntax.checkbox),
            task_done: hsla(syntax.task_done),
            highlight: hsla(syntax.highlight),
            comment: hsla(syntax.comment),
            frontmatter: hsla(syntax.frontmatter),
            frontmatter_key: hsla(syntax.frontmatter_key),
            block_id: hsla(syntax.block_id),
            fence_info: hsla(syntax.fence_info),
            code_keyword: hsla(syntax.code_keyword),
            code_string: hsla(syntax.code_string),
            code_number: hsla(syntax.code_number),
            code_comment: hsla(syntax.code_comment),
            markup: hsla(syntax.markup),
            headings: headings
                .0
                .map(|heading| (heading.size, FontWeight(f32::from(heading.weight)))),
            strike_thickness: px(stroke.text_line.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub background: Hsla,
    pub foreground: Hsla,
    pub caret: Hsla,
    /// El cursor de bloque deja ver el carácter que tapa.
    pub block_cursor: Hsla,
    /// El cursor de bloque en Visual.
    pub visual_cursor: Hsla,
    /// El cursor de un panel sin foco.
    pub unfocused_cursor: Hsla,
    pub selection: Hsla,
    /// Las coincidencias de la búsqueda activa.
    pub search: Hsla,
    /// La coincidencia donde está el cursor.
    pub search_current: Hsla,
    /// Fondo del margen de los números de línea.
    pub gutter: Hsla,
    pub gutter_foreground: Hsla,
    /// El número de la línea del cursor.
    pub gutter_current: Hsla,
    /// La letra de una mark.
    pub gutter_mark: Hsla,
    pub status_background: Hsla,
    pub status_foreground: Hsla,
    pub status_unfocused_foreground: Hsla,
    pub status_border: Hsla,
    /// La línea entre dos paneles.
    pub pane_border: Hsla,
    pub modified: Hsla,
    pub error: Hsla,
    pub picker_background: Hsla,
    pub picker_border: Hsla,
    pub picker_selected: Hsla,
    pub picker_hover: Hsla,
    pub picker_match: Hsla,
    pub picker_muted: Hsla,
    pub panel_background: Hsla,
    pub panel_foreground: Hsla,
    /// Adjuntos, encabezado y botones inactivos.
    pub panel_muted: Hsla,
    pub panel_selection: Hsla,
    pub panel_selection_unfocused: Hsla,
    pub panel_active: Hsla,
    pub panel_hover: Hsla,
    pub panel_border: Hsla,
    pub panel_folder: Hsla,
    pub panel_note: Hsla,
    /// Las letras que coinciden con el filtro.
    pub panel_match: Hsla,
    pub menu_background: Hsla,
    pub menu_border: Hsla,
    pub menu_hover: Hsla,
    pub font_family: SharedString,
    /// El texto de las notas (`text.editor`).
    pub editor_font_size: Pixels,
    /// Alto de línea del editor, relativo a su tamaño de letra.
    pub editor_line_height: f32,
    /// El texto de la interfaz (`text.ui_md`).
    pub ui_font_size: Pixels,
    pub ui_line_height: f32,
    /// Lo secundario de la interfaz (`text.ui_sm`): atajos, detalles y contadores.
    pub ui_small_font_size: Pixels,
    pub ui_small_line_height: f32,
    pub weight_normal: FontWeight,
    /// La negrita del markdown y las letras que coinciden en el picker.
    pub weight_strong: FontWeight,
    pub caret_width: Pixels,
    pub underline_thickness: Pixels,
    pub border_width: Pixels,
    pub pane_border_width: Pixels,
    /// Entre el borde del panel y el texto, con los números de línea.
    pub text_padding_x: Pixels,
    pub text_padding_y: Pixels,
    pub status_padding_x: Pixels,
    pub status_padding_y: Pixels,
    pub status_gap: Pixels,
    pub picker_width: Pixels,
    /// Del borde de arriba de la ventana al picker.
    pub picker_top: Pixels,
    /// Cuántas filas se ven antes de hacer scroll.
    pub picker_rows: u8,
    /// Lo mínimo de una fila del picker que se queda el detalle de la
    /// derecha cuando el texto no cabe.
    pub picker_detail_share: f32,
    pub picker_padding_x: Pixels,
    /// Arriba y abajo de la línea de texto de una fila.
    pub picker_row_padding_y: Pixels,
    pub picker_input_padding_y: Pixels,
    /// Entre el texto de una fila y su detalle, y antes del contador del input.
    pub picker_gap: Pixels,
    pub panel_width: Pixels,
    /// Alto de cada fila del árbol: todas iguales, para virtualizarlo.
    pub panel_row_height: Pixels,
    /// Sangría por nivel del árbol.
    pub panel_indent: Pixels,
    pub panel_padding_x: Pixels,
    pub panel_padding_y: Pixels,
    pub panel_gap: Pixels,
    pub panel_button_padding_x: Pixels,
    pub icon_size: Pixels,
    /// La barra de botones a la izquierda del panel.
    pub rail_width: Pixels,
    pub rail_button_size: Pixels,
    pub radius_sm: Pixels,
    pub radius_md: Pixels,
    pub radius_lg: Pixels,
    pub menu_width: Pixels,
    pub menu_padding: Pixels,
    pub menu_item_padding_x: Pixels,
    /// Una entrada del menú; con el texto secundario más grande, mide por él.
    pub menu_row_height: Pixels,
    pub tooltip_padding_x: Pixels,
    pub tooltip_padding_y: Pixels,
    /// Las columnas de las marks en el margen.
    pub gutter_mark_columns: usize,
    pub markdown: Markdown,
}

impl Global for Theme {}

impl Theme {
    /// Los colores y tamaños de `config`; si la familia de su fuente no
    /// está instalada, la primera de [`MONO_FONTS`] que sí.
    ///
    /// # Errors
    /// [`NoMonoFont`] si no hay ninguna.
    pub fn new(config: &config::Config, cx: &App) -> Result<Self, NoMonoFont> {
        let installed = cx.text_system().all_font_names();
        let font_family = pick_font(config.settings.font.family.as_deref(), &installed)?;
        Ok(Self::with_font(config, font_family))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "una línea por token del tema; partirla no aclara nada"
    )]
    fn with_font(config: &config::Config, font_family: SharedString) -> Self {
        let config::Theme {
            ui,
            cursor,
            syntax,
            headings,
            metrics,
            ..
        } = &config.theme;
        let (radius, stroke, text) = (&metrics.radius, &metrics.stroke, &metrics.text);
        let (rows, layout) = (&metrics.rows, &metrics.layout);
        let settings = &config.settings;
        let padding = settings.editor.padding;
        let font_size = settings.font.size.get();
        let ui_font = font_size * text.ui_md.size;
        let ui_small_font = font_size * text.ui_sm.size;
        Self {
            background: hsla(ui.background),
            foreground: hsla(ui.foreground),
            caret: hsla(cursor.insert),
            block_cursor: hsla(cursor.normal),
            visual_cursor: hsla(cursor.visual),
            unfocused_cursor: hsla(cursor.unfocused),
            selection: hsla(ui.selection),
            search: hsla(ui.search),
            search_current: hsla(ui.search_current),
            gutter: hsla(ui.gutter),
            gutter_foreground: hsla(ui.gutter_foreground),
            gutter_current: hsla(ui.gutter_current),
            gutter_mark: hsla(ui.gutter_mark),
            status_background: hsla(ui.status_background),
            status_foreground: hsla(ui.status_foreground),
            status_unfocused_foreground: hsla(ui.status_unfocused_foreground),
            status_border: hsla(ui.status_border),
            pane_border: hsla(ui.pane_border),
            modified: hsla(ui.modified),
            error: hsla(ui.error),
            picker_background: hsla(ui.picker_background),
            picker_border: hsla(ui.picker_border),
            picker_selected: hsla(ui.picker_selected),
            picker_hover: hsla(ui.picker_hover),
            picker_match: hsla(ui.picker_match),
            picker_muted: hsla(ui.picker_muted),
            panel_background: hsla(ui.panel_background),
            panel_foreground: hsla(ui.panel_foreground),
            panel_muted: hsla(ui.panel_muted),
            panel_selection: hsla(ui.panel_selection),
            panel_selection_unfocused: hsla(ui.panel_selection_unfocused),
            panel_active: hsla(ui.panel_active),
            panel_hover: hsla(ui.panel_hover),
            panel_border: hsla(ui.panel_border),
            panel_folder: hsla(ui.panel_folder),
            panel_note: hsla(ui.panel_note),
            panel_match: hsla(ui.panel_match),
            menu_background: hsla(ui.menu_background),
            menu_border: hsla(ui.menu_border),
            menu_hover: hsla(ui.menu_hover),
            font_family,
            editor_font_size: px(font_size * text.editor.size),
            editor_line_height: text.editor.line_height,
            ui_font_size: px(ui_font),
            ui_line_height: text.ui_md.line_height,
            ui_small_font_size: px(ui_small_font),
            ui_small_line_height: text.ui_sm.line_height,
            weight_normal: FontWeight(f32::from(text.normal.0)),
            weight_strong: FontWeight(f32::from(text.strong.0)),
            caret_width: px(stroke.caret.0),
            underline_thickness: px(stroke.text_line.0),
            border_width: px(stroke.border.0),
            pane_border_width: px(f32::from(settings.panes.border_width)),
            text_padding_x: px(f32::from(padding.horizontal)),
            text_padding_y: px(f32::from(padding.vertical)),
            status_padding_x: px(layout.status_padding_x.0),
            status_padding_y: px(layout.status_padding_y.0),
            status_gap: px(layout.status_gap.0),
            picker_width: px(layout.picker_width.0),
            picker_top: px(layout.picker_top.0),
            picker_rows: layout.picker_rows.0,
            picker_detail_share: layout.picker_detail_share.0,
            picker_padding_x: px(layout.picker_padding_x.0),
            picker_row_padding_y: px(rows.picker_padding.0),
            picker_input_padding_y: px(layout.picker_input_padding_y.0),
            picker_gap: px(layout.picker_gap.0),
            panel_width: px(f32::from(settings.panel.width.get())),
            panel_row_height: px((ui_font * rows.tree.0).round()),
            panel_indent: px(layout.panel_indent.0),
            panel_padding_x: px(layout.panel_padding_x.0),
            panel_padding_y: px(layout.panel_padding_y.0),
            panel_gap: px(layout.panel_gap.0),
            panel_button_padding_x: px(layout.panel_button_padding_x.0),
            icon_size: px((ui_font * layout.icon_size.0).round()),
            rail_width: px(layout.rail_width.0),
            rail_button_size: px(layout.rail_button_size.0),
            radius_sm: px(radius.sm.0),
            radius_md: px(radius.md.0),
            radius_lg: px(radius.lg.0),
            menu_width: px(layout.menu_width.0),
            menu_padding: px(layout.menu_padding.0),
            menu_item_padding_x: px(layout.menu_item_padding_x.0),
            menu_row_height: px((ui_font.max(ui_small_font) * rows.menu.0).round()),
            tooltip_padding_x: px(layout.tooltip_padding_x.0),
            tooltip_padding_y: px(layout.tooltip_padding_y.0),
            gutter_mark_columns: usize::from(layout.gutter_mark_columns.0),
            markdown: Markdown::new(syntax, headings, stroke),
        }
    }

    #[must_use]
    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// `element` con la letra de las notas.
    pub fn editor_text<E: Styled>(&self, element: E) -> E {
        element
            .text_size(self.editor_font_size)
            .line_height(relative(self.editor_line_height))
    }

    /// `element` con la letra de la interfaz.
    pub fn ui_text<E: Styled>(&self, element: E) -> E {
        element
            .text_size(self.ui_font_size)
            .line_height(relative(self.ui_line_height))
    }

    /// `element` con la letra de lo secundario: atajos, detalles y contadores.
    pub fn ui_small_text<E: Styled>(&self, element: E) -> E {
        element
            .text_size(self.ui_small_font_size)
            .line_height(relative(self.ui_small_line_height))
    }

    /// Alto de una línea de texto de la interfaz.
    #[must_use]
    pub fn ui_line(&self) -> Pixels {
        self.ui_font_size * self.ui_line_height
    }

    /// Alto de una fila del picker: su línea más alta (el texto o el detalle,
    /// que va en `ui_sm`) con aire arriba y abajo.
    #[must_use]
    pub fn picker_row_height(&self) -> Pixels {
        let detail = self.ui_small_font_size * self.ui_small_line_height;
        self.ui_line().max(detail) + self.picker_row_padding_y * 2.
    }

    /// El lado corto de los íconos provisionales del panel, que no son
    /// cuadrados; se van con los de Tabler (#41).
    #[must_use]
    pub fn icon_short_side(&self) -> Pixels {
        self.icon_size * ICON_SHAPE_RATIO
    }
}

/// Proporción de las formas de carpeta y nota mientras no hay íconos (#41).
const ICON_SHAPE_RATIO: f32 = 0.75;

fn hsla(color: config::Color) -> Hsla {
    Hsla::from(rgb(color.rgb)).opacity(color.alpha)
}

fn pick_font(configured: Option<&str>, installed: &[String]) -> Result<SharedString, NoMonoFont> {
    if let Some(family) = configured {
        if installed.iter().any(|name| name == family) {
            return Ok(SharedString::from(family.to_owned()));
        }
        log::warn!("la fuente `{family}` no está instalada; se usa otra monoespaciada");
    }
    first_installed(MONO_FONTS, installed).ok_or(NoMonoFont)
}

fn first_installed(candidates: &[&str], installed: &[String]) -> Option<SharedString> {
    candidates
        .iter()
        .find(|candidate| installed.iter().any(|name| name == *candidate))
        .map(|&family| SharedString::from(family))
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
