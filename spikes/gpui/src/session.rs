use std::path::PathBuf;
use std::time::Instant;

use gpui_kit::Global;

use crate::cli::View;
use crate::metrics::Metrics;

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub view: View,
    pub path: PathBuf,
    pub bytes: usize,
    pub lines: usize,
    pub bench: bool,
}

/// Estado de la corrida, accesible desde cualquier callback de GPUI.
#[derive(Debug)]
pub struct Session {
    pub info: SessionInfo,
    pub metrics: Metrics,
}

impl Session {
    pub fn new(info: SessionInfo, launched: Instant) -> Self {
        Self {
            info,
            metrics: Metrics::new(launched),
        }
    }
}

impl Global for Session {}
