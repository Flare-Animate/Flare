//! What the command line and the MCP server share: settings from what was asked for, laid over
//! the suggested ones, and a plan said as JSON, so a person or an agent sees what an import
//! makes before anything is read.

use flare_core::FrameRate;
use flare_document::Document;
use serde_json::{Value, json};

use super::{Plan, Settings, Source, TooLarge, VideoRefusal, megabytes};

/// What was asked for; each left out keeps the suggested value.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Asked {
    /// Pictures a second to take.
    pub fps: Option<f64>,
    /// The most of the video to read, in minutes.
    pub max_minutes: Option<f64>,
    pub max_side: Option<u32>,
    pub opacity: Option<u8>,
}

impl Asked {
    /// The suggested settings for `source` in `document`, with what was asked for laid over.
    pub fn settings(&self, document: &Document, source: &Source) -> Result<Settings, VideoRefusal> {
        let mut settings = Settings::suggested(document, source);
        if let Some(fps) = self.fps {
            settings.rate = FrameRate::from_per_second(fps).map_err(|_| {
                VideoRefusal::Setting("the frame rate to take pictures at is 0.01 to 240")
            })?;
        }
        if let Some(minutes) = self.max_minutes {
            settings.max_seconds = minutes * 60.0;
        }
        if let Some(side) = self.max_side {
            settings.max_side = side;
        }
        if let Some(opacity) = self.opacity {
            settings.opacity = opacity;
        }
        Ok(settings)
    }
}

/// The plan as JSON: what the video is, what the import makes, and when it does not fit, why
/// and what would.
pub fn plan_json(plan: &Plan) -> Value {
    let source = &plan.source;
    let too_large = plan.too_large.map(|why| match why {
        TooLarge::Frames { needed, room } => json!({
            "reason": why.to_string(), "frames": needed, "room_frames": room,
        }),
        TooLarge::Bytes { needed, room } => json!({
            "reason": why.to_string(), "bytes": needed, "room_bytes": room,
        }),
    });
    json!({
        "video": {
            "width": source.width,
            "height": source.height,
            "fps": source.rate.per_second(),
            "seconds": source.seconds,
        },
        "fps": plan.settings.rate.per_second(),
        "max_minutes": plan.settings.max_seconds / 60.0,
        "max_side": plan.settings.max_side,
        "opacity": plan.settings.opacity,
        "picture": { "width": plan.size.0, "height": plan.size.1 },
        "frames": plan.frames,
        "seconds_of_video": plan.seconds,
        "whole_video": plan.whole,
        "megabytes": megabytes(plan.bytes),
        "document_fps": plan.document_rate.per_second(),
        "animation_seconds": plan.length_seconds,
        "speed": plan.speed,
        "keeps_timing": plan.keeps_timing(),
        "fits": plan.fits(),
        "large": plan.is_large(),
        "too_large": too_large,
        "fitting_minutes": plan.fitting_seconds / 60.0,
        "fitting_side": plan.fitting_side,
    })
}
