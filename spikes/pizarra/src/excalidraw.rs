//! Un `.excalidraw` en memoria, para comparar formatos y para el esbozo del
//! importador (`src/bin/convertir.rs`). La pizarra no lo usa: su formato es
//! el de `formato.rs`.
//!
//! El JSON se guarda tal cual, con el orden de sus claves, y escribir da los
//! mismos bytes que el `JSON.stringify(datos, null, 2)` de Excalidraw.

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Number, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExcalidrawError {
    #[error("no es JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no es un archivo de Excalidraw: falta {0}")]
    Format(&'static str),
    #[error("no hay un elemento con id {0}")]
    Missing(String),
}

/// Lo que se lee de un elemento. Lo demás sigue en el JSON y se escribe igual.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub stroke_color: Option<String>,
    #[serde(default)]
    pub stroke_width: Option<f64>,
    #[serde(default)]
    pub is_deleted: bool,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub start_binding: Option<Binding>,
    #[serde(default)]
    pub end_binding: Option<Binding>,
    #[serde(default)]
    pub start_arrowhead: Option<String>,
    #[serde(default)]
    pub end_arrowhead: Option<String>,
    /// El título de un `frame`.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub angle: f64,
    /// Una flecha en codo (`elbowed` de Excalidraw 0.18).
    #[serde(default)]
    pub elbowed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub element_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct Read {
    pub elements: Vec<Element>,
    /// El `id` de cada elemento que no se entiende, o `#posición` si no tiene.
    pub unreadable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// La raíz; su `"elements"` vive aparte para editarlo sin buscarlo.
    root: Map<String, Value>,
    elements: Vec<Value>,
}

impl Scene {
    /// # Errors
    ///
    /// JSON inválido o sin `"type": "excalidraw"` y `"elements"`.
    pub fn parse(text: &str) -> Result<Self, ExcalidrawError> {
        let Value::Object(mut root) = serde_json::from_str(text)? else {
            return Err(ExcalidrawError::Format("el objeto raíz"));
        };
        if root.get("type").and_then(Value::as_str) != Some("excalidraw") {
            return Err(ExcalidrawError::Format("\"type\": \"excalidraw\""));
        }
        let Some(Value::Array(elements)) = root.get_mut("elements").map(Value::take) else {
            return Err(ExcalidrawError::Format("\"elements\""));
        };
        Ok(Self { root, elements })
    }

    /// Una escena vacía, como la que exporta excalidraw.com.
    #[must_use]
    pub fn new() -> Self {
        let mut root = Map::new();
        root.insert("type".into(), "excalidraw".into());
        root.insert("version".into(), 2.into());
        root.insert("source".into(), "https://excalidraw.com".into());
        root.insert("elements".into(), Value::Null);
        let mut state = Map::new();
        state.insert("gridSize".into(), 20.into());
        state.insert("viewBackgroundColor".into(), "#ffffff".into());
        root.insert("appState".into(), Value::Object(state));
        root.insert("files".into(), Value::Object(Map::new()));
        Self {
            root,
            elements: Vec::new(),
        }
    }

    /// Como `JSON.stringify(datos, null, 2)`: dos espacios, sin salto final.
    #[must_use]
    pub fn write(&self) -> String {
        // Serializar valores de serde_json a una cadena no falla.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Los elementos vivos, en orden de pintado.
    #[must_use]
    pub fn elements(&self) -> Vec<Element> {
        self.read().elements
    }

    /// Los elementos vivos y los que no se entienden (sin `id`, `type`, `x`
    /// o `y`, o con un campo de otro tipo), que siguen en el archivo y quien
    /// convierte tiene que poder listar.
    #[must_use]
    pub fn read(&self) -> Read {
        let mut read = Read::default();
        for (index, value) in self.elements.iter().enumerate() {
            match Element::deserialize(value) {
                Ok(element) if element.is_deleted => {}
                Ok(element) => read.elements.push(element),
                Err(_) => read.unreadable.push(
                    value
                        .get("id")
                        .and_then(Value::as_str)
                        .map_or_else(|| format!("#{index}"), str::to_owned),
                ),
            }
        }
        read
    }

    pub fn push(&mut self, element: Map<String, Value>) {
        self.elements.push(Value::Object(element));
    }

    /// Mueve una forma como Excalidraw: su texto contenido va con ella y las
    /// flechas pegadas mueven el extremo que la toca. Cada elemento tocado
    /// cambia además `version`, `versionNonce` y `updated`.
    ///
    /// # Errors
    ///
    /// Si no hay un elemento con ese id.
    pub fn move_with_bindings(
        &mut self,
        id: &str,
        dx: f64,
        dy: f64,
        now_ms: u64,
    ) -> Result<(), ExcalidrawError> {
        let moved = self.elements();
        if !moved.iter().any(|e| e.id == id) {
            return Err(ExcalidrawError::Missing(id.to_owned()));
        }
        for value in &mut self.elements {
            let Some(object) = value.as_object_mut() else {
                continue;
            };
            let Ok(element) = Element::deserialize(&Value::Object(object.clone())) else {
                continue;
            };
            let bound_to = |b: &Option<Binding>| b.as_ref().is_some_and(|b| b.element_id == id);
            let starts_here = bound_to(&element.start_binding);
            let ends_here = bound_to(&element.end_binding);
            if element.id == id || element.container_id.as_deref() == Some(id) {
                shift(object, "x", dx);
                shift(object, "y", dy);
            } else if starts_here || ends_here {
                move_arrow_ends(object, &element, starts_here, ends_here, dx, dy);
            } else {
                continue;
            }
            touch(object, now_ms);
        }
        Ok(())
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for Scene {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.root.len()))?;
        for (key, value) in &self.root {
            if key == "elements" {
                map.serialize_entry(key, &self.elements)?;
            } else {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

fn shift(object: &mut Map<String, Value>, key: &str, delta: f64) {
    let before = object.get(key).and_then(Value::as_f64).unwrap_or(0.0);
    object.insert(key.into(), number(before + delta));
}

/// Los puntos de una flecha son relativos a su `(x, y)`: mover el inicio
/// mueve el origen y compensa los demás puntos.
fn move_arrow_ends(
    object: &mut Map<String, Value>,
    arrow: &Element,
    start: bool,
    end: bool,
    dx: f64,
    dy: f64,
) {
    // Sin puntos no hay extremos que mover ni tamaño que recalcular.
    if arrow.points.is_empty() {
        return;
    }
    let mut points = arrow.points.clone();
    if start {
        shift(object, "x", dx);
        shift(object, "y", dy);
        for point in points.iter_mut().skip(1) {
            *point = [point[0] - dx, point[1] - dy];
        }
    }
    if end && let Some(last) = points.last_mut() {
        *last = [last[0] + dx, last[1] + dy];
    }
    let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().map(|p| (p[0], p[1])).unzip();
    let span = |v: &[f64]| {
        v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min)
    };
    object.insert("width".into(), number(span(&xs)));
    object.insert("height".into(), number(span(&ys)));
    let points = points
        .iter()
        .map(|[x, y]| Value::Array(vec![number(*x), number(*y)]))
        .collect();
    object.insert("points".into(), Value::Array(points));
}

fn touch(object: &mut Map<String, Value>, now_ms: u64) {
    let version = object.get("version").and_then(Value::as_u64).unwrap_or(0);
    object.insert("version".into(), (version + 1).into());
    let nonce = object
        .get("versionNonce")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    object.insert("versionNonce".into(), next_nonce(nonce).into());
    object.insert("updated".into(), now_ms.into());
}

/// Un número como lo escribe JavaScript: `100`, no `100.0`.
#[must_use]
pub fn number(value: f64) -> Value {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "solo se usa si es entero y cabe, que es lo que se comprueba"
    )]
    let integer = value as i64;
    #[expect(
        clippy::cast_precision_loss,
        reason = "se compara para saber si es exacto"
    )]
    let exact = value.abs() < 9e15 && (integer as f64).to_bits() == value.to_bits();
    if exact {
        return Value::Number(Number::from(integer));
    }
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

/// Excalidraw usa un entero aleatorio de 31 bits; aquí basta con que cambie.
fn next_nonce(nonce: u64) -> u64 {
    nonce
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
        >> 33
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que guarda excalidraw.com: dos espacios, una clave que el spike no
    /// conoce (`customData`) y cada punto en cuatro líneas.
    const FROM_EXCALIDRAW: &str = r##"{
  "type": "excalidraw",
  "version": 2,
  "source": "https://excalidraw.com",
  "elements": [
    {
      "id": "caja",
      "type": "rectangle",
      "x": 100,
      "y": 80.5,
      "width": 240,
      "height": 120,
      "angle": 0,
      "strokeColor": "#1e1e1e",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "index": "a0",
      "roundness": {
        "type": 3
      },
      "seed": 1468023541,
      "version": 7,
      "versionNonce": 1913449397,
      "isDeleted": false,
      "boundElements": [
        {
          "type": "arrow",
          "id": "flecha"
        }
      ],
      "updated": 1761400000000,
      "link": "[[ideas/pizarra]]",
      "locked": false,
      "customData": {
        "tuyo": "ñandú"
      }
    },
    {
      "id": "flecha",
      "type": "arrow",
      "x": 340,
      "y": 140,
      "width": 120,
      "height": 0,
      "angle": 0,
      "strokeColor": "#1e1e1e",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "dashed",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "index": "a1",
      "roundness": null,
      "seed": 7,
      "version": 3,
      "versionNonce": 5,
      "isDeleted": false,
      "boundElements": null,
      "updated": 1761400000000,
      "link": null,
      "locked": false,
      "points": [
        [
          0,
          0
        ],
        [
          120,
          0
        ]
      ],
      "lastCommittedPoint": null,
      "startBinding": {
        "elementId": "caja",
        "focus": 0,
        "gap": 1
      },
      "endBinding": null,
      "startArrowhead": null,
      "endArrowhead": "arrow",
      "elbowed": false
    }
  ],
  "appState": {
    "gridSize": 20,
    "viewBackgroundColor": "#ffffff"
  },
  "files": {}
}"##;

    fn changed_lines(before: &str, after: &str) -> Vec<String> {
        similar::TextDiff::from_lines(before, after)
            .iter_all_changes()
            .filter(|c| c.tag() == similar::ChangeTag::Insert)
            .map(|c| c.value().trim().to_owned())
            .collect()
    }

    #[test]
    fn rewrites_what_excalidraw_saved_byte_for_byte() {
        let scene = Scene::parse(FROM_EXCALIDRAW).unwrap();
        assert_eq!(scene.write(), FROM_EXCALIDRAW);
    }

    #[test]
    fn reads_what_is_needed_to_convert() {
        let elements = Scene::parse(FROM_EXCALIDRAW).unwrap().elements();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].link.as_deref(), Some("[[ideas/pizarra]]"));
        assert_eq!(elements[1].points, vec![[0.0, 0.0], [120.0, 0.0]]);
        assert_eq!(
            elements[1]
                .start_binding
                .as_ref()
                .map(|b| b.element_id.as_str()),
            Some("caja")
        );
        assert_eq!(elements[1].start_arrowhead, None);
    }

    #[test]
    fn moving_a_box_also_rewrites_the_arrow_glued_to_it() {
        let mut scene = Scene::parse(FROM_EXCALIDRAW).unwrap();
        scene
            .move_with_bindings("caja", 20.0, -0.5, 1_761_400_123_456)
            .unwrap();
        let after = scene.write();
        let lines = changed_lines(FROM_EXCALIDRAW, &after);
        assert!(lines.contains(&"\"x\": 120,".to_owned()), "{lines:?}");
        assert!(lines.contains(&"\"x\": 360,".to_owned()), "{lines:?}");
        assert!(lines.contains(&"\"version\": 8,".to_owned()), "{lines:?}");
        assert!(lines.contains(&"\"version\": 4,".to_owned()), "{lines:?}");
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.starts_with("\"updated\""))
                .count(),
            2
        );
    }

    #[test]
    fn rejects_what_is_not_excalidraw() {
        assert!(matches!(
            Scene::parse("[]"),
            Err(ExcalidrawError::Format(_))
        ));
        assert!(matches!(
            Scene::parse(r#"{"nodes": [], "edges": []}"#),
            Err(ExcalidrawError::Format(_))
        ));
        assert!(matches!(Scene::parse("{"), Err(ExcalidrawError::Json(_))));
    }

    #[test]
    fn a_deleted_element_is_kept_but_not_read() {
        let text = FROM_EXCALIDRAW.replacen("\"isDeleted\": false", "\"isDeleted\": true", 1);
        let scene = Scene::parse(&text).unwrap();
        assert_eq!(scene.elements().len(), 1);
        assert_eq!(scene.write(), text);
    }

    #[test]
    fn an_unreadable_element_is_listed_not_dropped() {
        let text = FROM_EXCALIDRAW.replacen("\"x\": 100,", "\"x\": \"cien\",", 1);
        let read = Scene::parse(&text).unwrap().read();
        assert_eq!(read.elements.len(), 1);
        assert_eq!(read.unreadable, ["caja"]);
    }

    #[test]
    fn moving_a_box_leaves_an_arrow_without_points_alone() {
        let text = FROM_EXCALIDRAW.replace(
            "\"points\": [\n        [\n          0,\n          0\n        ],\n        [\n          120,\n          0\n        ]\n      ],",
            "\"points\": [],",
        );
        let mut scene = Scene::parse(&text).unwrap();
        scene.move_with_bindings("caja", 20.0, 0.0, 1).unwrap();
        let after = scene.write();
        assert!(
            after.contains("\"width\": 120,"),
            "el ancho de la flecha no cambia"
        );
        assert!(!after.contains("null,\n      \"height\""), "{after}");
    }

    #[test]
    fn numbers_come_out_as_in_javascript() {
        assert_eq!(number(100.0).to_string(), "100");
        assert_eq!(number(-3.0).to_string(), "-3");
        assert_eq!(number(80.5).to_string(), "80.5");
        assert_eq!(number(0.1 + 0.2).to_string(), "0.30000000000000004");
    }
}
