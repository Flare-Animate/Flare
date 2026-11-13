//! Doing one job. Shared by the worker threads and the inline mode, so both give the same
//! answer.

use std::path::PathBuf;
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
                    Ok((bytes, flare_audio::Decoded::new(audio)))
                });
            Done::SoundImported { id, path, result }
        }
        AudioJob::ImportBytes { id, name, bytes } => {
            let path = std::path::PathBuf::from(name);
            let result = if bytes.len() > flare_audio::MAX_INPUT_BYTES {
                Err(flare_audio::AudioError::InputTooLarge.to_string())
            } else {
                flare_audio::decode(&bytes)
                    .map(|audio| (bytes, flare_audio::Decoded::new(audio)))
                    .map_err(|error| error.to_string())
            };
            Done::SoundImported { id, path, result }
        }
        AudioJob::Decode { sound, data } => Done::SoundDecoded {
            sound,
            result: flare_audio::decode(&data)
                .map(flare_audio::Decoded::new)
                .map_err(|e| e.to_string()),
        },
    }
}

/// Runs `work` with this thread's font database, built the first time it is needed.
fn with_engine<T>(fonts: &ExtraFonts, work: impl FnOnce(&mut flare_text::TextEngine) -> T) -> T {
    TEXT_ENGINE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let engine = slot.get_or_insert_with(|| {
            let mut engine = flare_text::TextEngine::new();
            for font in fonts.iter() {
                engine.register_font(font.clone());
            }
            engine
        });
        work(engine)
    })
}

pub fn run_text(job: &TextJob, fonts: &ExtraFonts) -> Done {
    match job {
        TextJob::Set {
            id,
            source,
            shape,
            fill,
        } => Done::Text {
            id: *id,
            result: text_shape(source, *shape, *fill, fonts),
        },
        TextJob::Families => Done::Families(font_families(fonts)),
        TextJob::Preview {
            id,
            text,
            style,
            origin,
            cursor,
            shape,
            fill,
        } => Done::TextPreview {
            id: *id,
            result: with_engine(fonts, |engine| {
                engine
                    .preview(text, style, *origin, *cursor, *shape, *fill)
                    .map_err(|e| e.to_string())
            }),
        },
    }
}

/// The shape `source` sets, with identity `shape` and `fill`. The first call on a thread builds
/// the font database, which scans the system's fonts.
pub fn text_shape(
    source: &flare_document::TextSource,
    shape: flare_core::ShapeId,
    fill: flare_core::Rgba8,
    fonts: &ExtraFonts,
) -> Result<flare_document::Shape, String> {
    with_engine(fonts, |engine| {
        engine.shape(source, shape, fill).map_err(|e| e.to_string())
    })
}

/// The font families there are to set text in, sorted.
pub fn font_families(fonts: &ExtraFonts) -> Vec<String> {
    with_engine(fonts, flare_text::TextEngine::families)
}

/// Draws the frame.
pub fn run_render(job: &RenderJob) -> Done {
    Done::Rendered {
        id: job.id,
        result: draw(job),
    }
}

fn draw(job: &RenderJob) -> Result<Rendered, String> {
    let evaluate = |frame: u32| {
        if job.through_camera {
            flare_eval::evaluate_frame(&job.document, job.scene, frame)
        } else {
            flare_eval::evaluate_frame_without_camera(&job.document, job.scene, frame)
        }
    };
    let mut current = evaluate(job.frame).map_err(|e| e.to_string())?;
    if let Some(preview) = &job.text_preview {
        show_text_preview(&mut current, preview);
    }
    // A ghost that fails to evaluate is simply left out; the current frame matters.
    let tint = |frame: u32| {
        job.tints
            .map(|[before, after]| if frame < job.frame { before } else { after })
    };
    let evaluated: Vec<(flare_eval::Frame<'_>, f32, Option<flare_core::Rgba8>)> = job
        .ghosts
        .iter()
        .filter_map(|(frame, opacity)| evaluate(*frame).ok().map(|e| (e, *opacity, tint(*frame))))
        .collect();
    let ghosts: Vec<(&flare_eval::Frame<'_>, f32, Option<flare_core::Rgba8>)> =
        evaluated.iter().map(|(e, o, t)| (e, *o, *t)).collect();
    let [width, height] = job.size;
    if job.outline_mode {
        // One pixel of the picture, in stage units, so lines stay thin however far in you are.
        let pixel = job.viewport.width / f64::from(width.max(1));
        return flare_render::render_outline_mode_viewport(
            &current,
            &job.viewport,
            width,
            height,
            job.background,
            pixel,
        )
        .map(Rendered::Straight)
        .map_err(|e| e.to_string());
    }
    // A transparent layer is a view aid: the stage fades it, and only the stage comes here.
    if flare_render::has_view_alpha(&current) {
        return stage_with_view_alpha(job, &ghosts, &current).map_err(|e| e.to_string());
    }
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
        let image = if job.outlines {
            // One pixel of the picture, in stage units.
            let pixel = job.viewport.width / f64::from(width.max(1));
            flare_render::render_onion_outlines_viewport(
                &ghosts,
                &current,
                &job.viewport,
                width,
                height,
                job.background,
                pixel,
            )
        } else {
            flare_render::render_onion_viewport(
                &ghosts,
                &current,
                &job.viewport,
                width,
                height,
                job.background,
            )
        };
        image.map(Rendered::Straight).map_err(|e| e.to_string())
    }
}

/// The frame with its transparent layers faded, over the ghosts when there are any.
fn stage_with_view_alpha(
    job: &RenderJob,
    ghosts: &[(&flare_eval::Frame<'_>, f32, Option<flare_core::Rgba8>)],
    current: &flare_eval::Frame<'_>,
) -> Result<Rendered, flare_render::RenderError> {
    let [width, height] = job.size;
    if ghosts.is_empty() {
        return flare_render::render_stage_viewport(
            current,
            &job.viewport,
            width,
            height,
            job.background,
        )
        .map(Rendered::Straight);
    }
    let empty = flare_eval::Frame::default();
    let mut image = if job.outlines {
        let pixel = job.viewport.width / f64::from(width.max(1));
        flare_render::render_onion_outlines_viewport(
            ghosts,
            &empty,
            &job.viewport,
            width,
            height,
            job.background,
            pixel,
        )?
    } else {
        flare_render::render_onion_viewport(
            ghosts,
            &empty,
            &job.viewport,
            width,
            height,
            job.background,
        )?
    };
    image.composite(&flare_render::render_stage_viewport(
        current,
        &job.viewport,
        width,
        height,
        flare_core::Rgba8::TRANSPARENT,
    )?)?;
    Ok(Rendered::Straight(image))
}

/// An import's outcome as the document and one line per thing that was not carried over.
fn imported(
    result: Result<flare_import::Imported, flare_import::ImportError>,
) -> Result<(flare_document::Document, Vec<String>), String> {
    result
        .map(|imported| {
            let notes = imported
                .diagnostics
                .iter()
                .map(|d| format!("{:?} {}: {}", d.severity, d.location, d.message))
                .collect();
            (imported.document, notes)
        })
        .map_err(|e| e.to_string())
}

pub fn run_io(job: IoJob) -> Done {
    match job {
        IoJob::Save { path, document } => {
            let result = crate::save(&document, &path).map_err(|e| e.to_string());
            Done::Saved {
                path,
                document,
                result,
            }
        }
        IoJob::Open { id, path, recovery } => {
            // A recovery file is a project snapshot whatever its name; a file the person opens
            // is read as the format its name says.
            let result = if recovery {
                flare_storage::load(&path)
                    .map(|document| crate::Loaded {
                        document,
                        path: path.clone(),
                        backed: false,
                        notes: Vec::new(),
                    })
                    .map_err(|e| e.to_string())
            } else {
                crate::load(&path).map_err(|e| e.to_string())
            };
            Done::Opened {
                id,
                path,
                recovery,
                result,
            }
        }
        IoJob::Import { id, path } => {
            let result = imported(flare_import::import_path(&path));
            Done::Imported { id, path, result }
        }
        IoJob::ImportBytes { id, name, bytes } => Done::Imported {
            id,
            path: PathBuf::from(&name),
            result: imported(
                if flare_import::foreign_kind(std::path::Path::new(&name)).is_some() {
                    flare_import::import_foreign_bytes(std::path::Path::new(&name), bytes)
                } else {
                    flare_import::import_bytes(bytes)
                },
            ),
        },
        IoJob::Autosave { id, dir, document } => Done::Autosaved {
            id,
            result: flare_storage::save_recovery(&document, &dir).map_err(|e| e.to_string()),
        },
    }
}

/// Puts the text being typed where the text it edits is, or on top of the scene if it is new.
fn show_text_preview(frame: &mut flare_eval::Frame<'_>, preview: &crate::jobs::TextPreviewRender) {
    use std::borrow::Cow;
    let position = preview
        .replaces
        .and_then(|id| frame.shapes.iter().position(|d| d.shape.id() == id));
    match (position, &preview.shape) {
        (Some(at), Some(shape)) => frame.shapes[at].shape = Cow::Owned(shape.clone()),
        (Some(at), None) => {
            frame.shapes.remove(at);
        }
        (None, Some(shape)) => frame.shapes.push(flare_eval::DrawnShape {
            path: Vec::new(),
            shape: Cow::Owned(shape.clone()),
            phase: 0,
            transform: glamx::DAffine2::IDENTITY,
            clips: Vec::new(),
            outline: flare_core::Rgba8::TRANSPARENT,
            view_alpha: 1.0,
        }),
        (None, None) => {}
    }
}
