//! The project as a Flare document: the stage, the frame rate, the root clip's timeline as the one
//! scene, and the symbols, bitmaps and kept source the scene uses.

use std::collections::HashMap;

use flare_core::{DocumentId, FrameRate, Rgba8, SceneId};
use flare_document::{Anchor, Document, Scene, Stage};

use super::color;
use super::ctx::Ctx;
use super::error::WickError;
use super::json::Parsed;
use super::timeline;
use crate::xfl::{Diagnostic, Severity};

pub(super) fn build(
    parsed: &Parsed,
    files: &HashMap<String, Vec<u8>>,
) -> Result<(Document, Vec<Diagnostic>), WickError> {
    let project = &parsed.project;
    let objects = &parsed.objects;
    let root = root_clip(parsed)?;
    let mut ctx = Ctx::new(objects, files);
    let root_obj = objects.get(&root)?;
    let root_timeline = objects.child_of_class(root_obj, "Timeline")?;
    ctx.scripts(&root, root_obj, Anchor::Document, "the root clip");
    let timeline = timeline::build(&mut ctx, &root_timeline, Anchor::Document)?;
    let scene = Scene::new(SceneId::new(), project.name.as_str(), timeline);

    let background = match project.background.as_deref() {
        None => Rgba8::WHITE,
        Some(text) => color::css(text).unwrap_or_else(|| {
            ctx.note(
                &root,
                "background",
                Severity::Warning,
                "the background colour is not read; white is used",
            );
            Rgba8::WHITE
        }),
    };
    let stage = Stage::new(project.width, project.height, background)
        .map_err(|_| WickError::BadField("width or height"))?;
    let rate =
        FrameRate::new(project.framerate, 1).map_err(|_| WickError::BadField("framerate"))?;

    let mut diagnostics = vec![Diagnostic {
        severity: Severity::Info,
        location: project.name.clone(),
        message: format!(
            "Wick project: {}x{} at {} fps, {} clips and buttons, {} bitmaps",
            project.width,
            project.height,
            project.framerate,
            ctx.symbols.len(),
            ctx.library.len(),
        ),
    }];
    diagnostics.extend(ctx.diagnostics(&project.name));
    let symbols = std::mem::take(&mut ctx.symbols);
    let attachments = std::mem::take(&mut ctx.attachments);
    let bitmaps = ctx.library.into_bitmaps();
    let document = Document::new(DocumentId::new(), rate, vec![scene], symbols)
        .map_err(|d| WickError::Invalid(format!("{d:?}")))?
        .with_stage(stage)
        .with_bitmaps(bitmaps)
        .map_err(|d| WickError::Invalid(format!("{d:?}")))?
        .with_attachments(attachments)
        .map_err(|d| WickError::Invalid(format!("{d:?}")))?;
    Ok((document, diagnostics))
}

/// The root clip: the focus when it is a clip, otherwise the project's first clip.
fn root_clip(parsed: &Parsed) -> Result<String, WickError> {
    let objects = &parsed.objects;
    if let Some(focus) = parsed
        .project
        .focus
        .as_deref()
        .filter(|f| objects.class(f) == "Clip")
    {
        return Ok(focus.to_owned());
    }
    parsed
        .project
        .children
        .iter()
        .find(|id| objects.class(id) == "Clip")
        .cloned()
        .ok_or(WickError::NoRootClip)
}
