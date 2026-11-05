//! A span's elements to and from their stored form.

use crate::StorageError;
use flare_core::{DocumentPoint, InstanceId, Rgba8, ShapeId, SymbolId, Transform2D};
use flare_document::{Cap, Content, Gradient, Instance, Join, Shape, Stroke, StrokeStyle};
use glamx::DAffine2;

use super::timeline::{
    ContentWire, GradientWire, SegmentWire, StrokeWire, TextWire, cap_from, cap_name, join_from,
    join_name,
};
use super::uuid;

pub(super) fn wire_content(content: &Content) -> ContentWire {
    match content {
        Content::Empty => ContentWire::Empty,
        Content::Shape(shape) => ContentWire::Shape {
            id: shape.id().as_uuid().to_string(),
            path: shape.path().iter().map(SegmentWire::of).collect(),
            rgba: shape.fill().to_array(),
            stroke: shape.stroke().map(|stroke| StrokeWire {
                width: stroke.width(),
                rgba: stroke.color().to_array(),
                cap: (stroke.cap() != Cap::Round).then(|| cap_name(stroke.cap()).to_owned()),
                join: (stroke.join() != Join::Round).then(|| join_name(stroke.join()).to_owned()),
                style: (stroke.style() != StrokeStyle::Solid)
                    .then(|| stroke.style().name().to_owned()),
            }),
            text: shape.text().map(TextWire::of),
            gradient: shape.gradient().map(|g| GradientWire {
                from: [g.from().as_vec().x, g.from().as_vec().y],
                to: [g.to().as_vec().x, g.to().as_vec().y],
                start: g.start().to_array(),
                end: g.end().to_array(),
            }),
        },
        Content::Instance(instance) => ContentWire::Instance {
            id: instance.id().as_uuid().to_string(),
            symbol: instance.symbol().as_uuid().to_string(),
            matrix: instance.transform().as_affine().to_cols_array(),
            play: Some(instance.playback().mode.name().to_owned()),
            first_frame: instance.playback().first_frame,
        },
        Content::Bitmap(bitmap) => ContentWire::Bitmap {
            id: bitmap.id().as_uuid().to_string(),
            bitmap: bitmap.bitmap().as_uuid().to_string(),
            matrix: bitmap.transform().as_affine().to_cols_array(),
            opacity: bitmap.opacity(),
        },
        Content::Camera(camera) => ContentWire::Camera {
            center: camera.center().as_vec().to_array(),
            zoom: camera.zoom(),
            rotation: camera.rotation(),
        },
    }
}

pub(super) fn build_content(content: ContentWire) -> Result<Content, StorageError> {
    let content = match content {
        ContentWire::Empty => Content::Empty,
        ContentWire::Shape {
            id,
            path,
            rgba,
            stroke,
            gradient,
            text,
        } => {
            let path = path
                .iter()
                .map(SegmentWire::build)
                .collect::<Result<_, _>>()?;
            let fill = Rgba8::new(rgba[0], rgba[1], rgba[2], rgba[3]);
            let corrupt =
                |error: flare_document::ShapeError| StorageError::Corrupt(error.to_string());
            let mut shape =
                Shape::new(ShapeId::from_uuid(uuid(&id)?), path, fill).map_err(corrupt)?;
            if let Some(stroke) = stroke {
                let [r, g, b, a] = stroke.rgba;
                let cap = cap_from(stroke.cap.as_deref())?;
                let join = join_from(stroke.join.as_deref())?;
                let style = match stroke.style.as_deref() {
                    None => StrokeStyle::Solid,
                    Some(name) => StrokeStyle::parse(name).ok_or_else(|| {
                        StorageError::Corrupt(format!("unknown stroke style {name:?}"))
                    })?,
                };
                let stroke = Stroke::new(stroke.width, Rgba8::new(r, g, b, a))
                    .map_err(corrupt)?
                    .with_cap(cap)
                    .with_join(join)
                    .with_style(style);
                shape = shape.with_stroke(stroke);
            }
            if let Some(g) = gradient {
                let point = |p: [f64; 2]| {
                    DocumentPoint::new(p[0], p[1])
                        .map_err(|_| StorageError::Corrupt("non-finite gradient".into()))
                };
                let color = |c: [u8; 4]| Rgba8::new(c[0], c[1], c[2], c[3]);
                let gradient =
                    Gradient::linear(point(g.from)?, point(g.to)?, color(g.start), color(g.end))
                        .map_err(|error| StorageError::Corrupt(error.to_string()))?;
                shape = shape.with_gradient(gradient);
            }
            if let Some(text) = text {
                shape = shape.with_text(text.build()?);
            }
            Content::Shape(shape)
        }
        ContentWire::Instance {
            id,
            symbol,
            matrix,
            play,
            first_frame,
        } => {
            let transform = Transform2D::new(DAffine2::from_cols_array(&matrix))
                .map_err(|_| StorageError::Corrupt("non-finite transform".into()))?;
            // No name means the default, a loop; a name that is not a mode is a damaged
            // file, not something to guess at.
            let mode = match play.as_deref() {
                None => flare_document::PlayMode::default(),
                Some(name) => flare_document::PlayMode::from_name(name)
                    .ok_or_else(|| StorageError::Corrupt(format!("unknown play mode {name:?}")))?,
            };
            Content::Instance(
                Instance::new(
                    InstanceId::from_uuid(uuid(&id)?),
                    SymbolId::from_uuid(uuid(&symbol)?),
                    transform,
                )
                .with_playback(flare_document::Playback { mode, first_frame }),
            )
        }
        ContentWire::Bitmap {
            id,
            bitmap,
            matrix,
            opacity,
        } => {
            let transform = Transform2D::new(DAffine2::from_cols_array(&matrix))
                .map_err(|_| StorageError::Corrupt("non-finite transform".into()))?;
            Content::Bitmap(
                flare_document::BitmapInstance::new(
                    InstanceId::from_uuid(uuid(&id)?),
                    flare_core::BitmapId::from_uuid(uuid(&bitmap)?),
                    transform,
                )
                .with_opacity(opacity),
            )
        }
        ContentWire::Camera {
            center,
            zoom,
            rotation,
        } => {
            let center = DocumentPoint::new(center[0], center[1])
                .map_err(|_| StorageError::Corrupt("non-finite camera".into()))?;
            Content::Camera(
                flare_document::Camera::new(center, zoom, rotation)
                    .map_err(|e| StorageError::Corrupt(e.to_string()))?,
            )
        }
    };
    Ok(content)
}
