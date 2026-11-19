//! Rendering bounded, portable shader graphs to ordinary Flare images.

use flare_core::ShaderGraph;

use crate::{Image, RenderError};

pub const MAX_SHADER_STACK: usize = 16;

/// Errors while evaluating a graph into an image.
#[derive(Debug, thiserror::Error)]
pub enum ShaderRenderError {
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error(transparent)]
    Graph(#[from] flare_core::ShaderGraphError),
    #[error("shader stacks are limited to {MAX_SHADER_STACK} graphs")]
    TooManyEffects,
}

/// Evaluates a graph at each pixel center. The image is bounded by the same limits as all renders.
pub fn render_shader(
    graph: &ShaderGraph,
    width: u32,
    height: u32,
    time: f32,
) -> Result<Image, ShaderRenderError> {
    evaluate_shader(graph, width, height, time, None)
}

/// Applies a graph to each source pixel, preserving the source image until all pixels succeed.
pub fn apply_shader(
    graph: &ShaderGraph,
    source: &Image,
    time: f32,
) -> Result<Image, ShaderRenderError> {
    evaluate_shader(graph, source.width, source.height, time, Some(source))
}

/// Applies graphs in order, feeding each result into the next graph's source-color inputs.
pub fn apply_shader_stack(
    graphs: &[&ShaderGraph],
    source: &Image,
    time: f32,
) -> Result<Image, ShaderRenderError> {
    if graphs.len() > MAX_SHADER_STACK {
        return Err(ShaderRenderError::TooManyEffects);
    }
    let mut image = source.clone();
    for graph in graphs {
        image = apply_shader(graph, &image, time)?;
    }
    Ok(image)
}

fn evaluate_shader(
    graph: &ShaderGraph,
    width: u32,
    height: u32,
    time: f32,
    source: Option<&Image>,
) -> Result<Image, ShaderRenderError> {
    let pixels = u64::from(width) * u64::from(height);
    if !(1..=crate::MAX_DIMENSION).contains(&width)
        || !(1..=crate::MAX_DIMENSION).contains(&height)
        || pixels > crate::limits::MAX_PIXELS
    {
        return Err(RenderError::InvalidSize(width, height).into());
    }
    let mut rgba = Vec::with_capacity(pixels as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let uv = [
                (x as f32 + 0.5) / width as f32,
                (y as f32 + 0.5) / height as f32,
            ];
            let source_color = source.map_or([0.0; 4], |image| {
                let offset = (y as usize * width as usize + x as usize) * 4;
                let pixel = &image.rgba[offset..offset + 4];
                [
                    f32::from(pixel[0]) / 255.0,
                    f32::from(pixel[1]) / 255.0,
                    f32::from(pixel[2]) / 255.0,
                    f32::from(pixel[3]) / 255.0,
                ]
            });
            let color = graph.evaluate_with_source(uv, time, source_color)?;
            rgba.extend(color.map(|channel| (channel * 255.0).round() as u8));
        }
    }
    Ok(Image::from_rgba(width, height, rgba)?)
}

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
