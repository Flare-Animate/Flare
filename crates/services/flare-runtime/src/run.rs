//! Doing one job. Shared by the worker threads and the inline mode, so both give the same
//! answer.

use std::sync::Arc;

use crate::jobs::{AudioJob, Done, IoJob, RenderJob, Rendered, TextJob};

/// Fonts a front end wants known by name in addition to the system's (tests use this so they do
/// not depend on what a machine has installed).
pub type ExtraFonts = Arc<Vec<Vec<u8>>>;

thread_local! {
    /// The font database of this thread, built the first time text is asked for.
    static TEXT_ENGINE: std::cell::RefCell<Option<flare_text::TextEngine>> =
        const { std::cell::RefCell::new(None) };
}

pub fn run_audio(job: AudioJob) -> Done {
    match job {
        AudioJob::Import { id, path } => {
            let result = std::fs::metadata(&path)
                .map_err(|e| e.to_string())
                .and_then(|meta| {
                    if meta.len() > flare_audio::MAX_INPUT_BYTES as u64 {
                        Err(flare_audio::AudioError::InputTooLarge.to_string())
                    } else {
                        std::fs::read(&path).map_err(|e| e.to_string())
                    }
                })
                .and_then(|bytes| {
                    let audio = flare_audio::decode(&bytes).map_err(|e| e.to_string())?;
                    Ok((bytes, Arc::new(audio)))
                });
            Done::SoundImported { id, path, result }
        }
        AudioJob::Decode { sound, data } => Done::SoundDecoded {
            sound,
            result: flare_audio::decode(&data)
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        },
    }
}

pub fn run_text(job: &TextJob, fonts: &ExtraFonts) -> Done {
    let result = TEXT_ENGINE.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(|| {
                let mut engine = flare_text::TextEngine::new();
                for font in fonts.iter() {
                    engine.register_font(font.clone());
                }
                engine
            })
            .outline(&job.text, &job.style)
            .map_err(|e| e.to_string())
    });
    Done::Text { id: job.id, result }
}

/// Draws the frame.
pub fn run_render(job: &RenderJob) -> Done {
    Done::Rendered {
        id: job.id,
        result: draw(job),
    }
}

fn draw(job: &RenderJob) -> Result<Rendered, String> {
    let current = flare_eval::evaluate_frame(&job.document, job.scene, job.frame)
        .map_err(|e| e.to_string())?;
    // A ghost that fails to evaluate is simply left out; the current frame matters.
    let evaluated: Vec<(flare_eval::Frame<'_>, f32)> = job
        .ghosts
        .iter()
        .filter_map(|(frame, opacity)| {
            flare_eval::evaluate_frame(&job.document, job.scene, *frame)
                .ok()
                .map(|e| (e, *opacity))
        })
        .collect();
    let ghosts: Vec<(&flare_eval::Frame<'_>, f32)> =
        evaluated.iter().map(|(e, o)| (e, *o)).collect();
    let [width, height] = job.size;
    if ghosts.is_empty() {
        flare_render::render_viewport_premultiplied(
            &current,
            &job.viewport,
            width,
            height,
            job.background,
        )
        .map(Rendered::Premultiplied)
        .map_err(|e| e.to_string())
    } else {
        flare_render::render_onion_viewport(
            &ghosts,
            &current,
            &job.viewport,
            width,
            height,
            job.background,
        )
        .map(Rendered::Straight)
        .map_err(|e| e.to_string())
    }
}

pub fn run_io(job: IoJob) -> Done {
    match job {
        IoJob::Save { path, document } => {
            let result = flare_storage::save(&document, &path).map_err(|e| e.to_string());
            Done::Saved {
                path,
                document,
                result,
            }
        }
        IoJob::Open { id, path, recovery } => {
            let result = flare_storage::load(&path).map_err(|e| e.to_string());
            Done::Opened {
                id,
                path,
                recovery,
                result,
            }
        }
        IoJob::Import { id, path } => {
            let result = flare_import::import_path(&path)
                .map(|imported| {
                    let notes = imported
                        .diagnostics
                        .iter()
                        .map(|d| format!("{:?} {}: {}", d.severity, d.location, d.message))
                        .collect();
                    (imported.document, notes)
                })
                .map_err(|e| e.to_string());
            Done::Imported { id, path, result }
        }
        IoJob::Autosave { id, dir, document } => Done::Autosaved {
            id,
            result: flare_storage::save_recovery(&document, &dir).map_err(|e| e.to_string()),
        },
    }
}
