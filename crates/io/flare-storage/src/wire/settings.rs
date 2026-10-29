//! Stage and settings records.

use flare_document::Document;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Settings {
    pub(super) frame_rate: (u32, u32),
    /// Absent in files written before stages existed; those load with the default stage.
    #[serde(default)]
    pub(super) stage: Option<StageWire>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct StageWire {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) background: [u8; 4],
}

impl Settings {
    pub(crate) fn of(document: &Document) -> Self {
        let rate = document.frame_rate();
        let stage = document.stage();
        Self {
            frame_rate: (rate.numerator(), rate.denominator()),
            stage: Some(StageWire {
                width: stage.width(),
                height: stage.height(),
                background: stage.background().to_array(),
            }),
        }
    }
}
