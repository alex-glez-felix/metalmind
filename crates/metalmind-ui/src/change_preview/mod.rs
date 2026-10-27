//! La vista previa de un cambio en el vault, sin saber de qué: las
//! ediciones, archivos nuevos y renombres de un plan, con sus avisos. Cada
//! fila se puede quitar y lo que queda es lo que se aplica. La usan
//! renombrar o fusionar tags (05 §5) y renombrar una nota con sus links
//! (03 §4).

mod preview;
mod view;

pub use self::preview::{Blocked, ChangePreview, Notice};
pub use self::view::{ChangePreviewEvent, ChangePreviewView};
