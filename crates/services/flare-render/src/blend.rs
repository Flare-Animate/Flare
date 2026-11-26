//! A layer's blend mode in the rasterizer, and laying one picture over another by it.

use flare_document::LayerBlend;
use vello_cpu::peniko::{BlendMode, Compose, ImageAlphaType, Mix};
use vello_cpu::{Pixmap, RenderContext, Resources};

use crate::image::{Image, unpremultiply_in_place};
use crate::{RenderError, effects};

/// The rasterizer's mode for `blend`. Add is the colors summed (Plus) rather than a mix.
pub(crate) fn mode(blend: LayerBlend) -> BlendMode {
    let mix = match blend {
        LayerBlend::Normal | LayerBlend::Add => Mix::Normal,
        LayerBlend::Multiply => Mix::Multiply,
        LayerBlend::Screen => Mix::Screen,
        LayerBlend::Overlay => Mix::Overlay,
        LayerBlend::Darken => Mix::Darken,
        LayerBlend::Lighten => Mix::Lighten,
        LayerBlend::ColorDodge => Mix::ColorDodge,
        LayerBlend::ColorBurn => Mix::ColorBurn,
        LayerBlend::HardLight => Mix::HardLight,
        LayerBlend::SoftLight => Mix::SoftLight,
        LayerBlend::Difference => Mix::Difference,
        LayerBlend::Exclusion => Mix::Exclusion,
        LayerBlend::Hue => Mix::Hue,
        LayerBlend::Saturation => Mix::Saturation,
        LayerBlend::Color => Mix::Color,
        LayerBlend::Luminosity => Mix::Luminosity,
    };
    let compose = if blend == LayerBlend::Add {
        Compose::Plus
    } else {
        Compose::SrcOver
    };
    BlendMode::new(mix, compose)
}

/// The image as a premultiplied picture for the rasterizer.
fn pixmap(image: &Image) -> Result<Pixmap, RenderError> {
    let invalid = || RenderError::InvalidSize(image.width, image.height);
    let w = u16::try_from(image.width).map_err(|_| invalid())?;
    let h = u16::try_from(image.height).map_err(|_| invalid())?;
    let mut out = Pixmap::new(w, h);
    for (px, rgba) in out.data_mut().iter_mut().zip(image.rgba.as_chunks::<4>().0) {
        let a = u16::from(rgba[3]);
        let mul = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
        px.r = mul(rgba[0]);
        px.g = mul(rgba[1]);
        px.b = mul(rgba[2]);
        px.a = rgba[3];
    }
    out.recompute_may_have_transparency();
    Ok(out)
}

/// Lays `top` over `bottom` mixed by `blend`, as the rasterizer mixes a layer with that mode.
/// Sizes must match.
pub(crate) fn composite(
    bottom: &mut Image,
    top: &Image,
    blend: LayerBlend,
) -> Result<(), RenderError> {
    if blend.is_normal() {
        return bottom.composite(top);
    }
    if (bottom.width, bottom.height) != (top.width, top.height) {
        return Err(RenderError::InvalidSize(top.width, top.height));
    }
    let (width, height) = (bottom.width, bottom.height);
    let under = pixmap(bottom)?;
    let over = pixmap(top)?;
    let (w, h) = (under.width(), under.height());
    let mut ctx = RenderContext::new(w, h);
    effects::lay(&mut ctx, under, width, height);
    ctx.push_blend_layer(mode(blend));
    effects::lay(&mut ctx, over, width, height);
    ctx.pop_layer();
    ctx.flush();
    let mut out = Pixmap::new(w, h);
    ctx.render(&mut out, &mut Resources::new());
    let translucent = out.may_have_transparency();
    let mut rgba = out.take_rgba8(ImageAlphaType::AlphaPremultiplied);
    if translucent {
        unpremultiply_in_place(&mut rgba);
    }
    bottom.rgba = rgba;
    Ok(())
}
