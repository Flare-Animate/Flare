//! The camera system. The C++ scaffold's camera follows its target with a lerp and clamps it to
//! bounds (both were commented out) and maps the world to the screen with a viewport-centred
//! transform. The port does both: the camera's own transform is where it looks, and the view
//! subtracts that position, which the C++ transform left out.

use crate::limits::MAX_COORD;
use crate::system::StepContext;
use crate::{Camera, Component, ComponentKind, EntityId, GameError, Transform, World};

/// Where a camera looks: its centre in world pixels, its zoom, and half its viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
    pub half_width: f64,
    pub half_height: f64,
}

impl View {
    /// The screen position of a world point, in viewport pixels from the top-left corner.
    pub fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        (
            (x - self.center_x) * self.zoom + self.half_width,
            (y - self.center_y) * self.zoom + self.half_height,
        )
    }
}

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    let dt = ctx.dt;
    let cameras: Vec<EntityId> = world
        .entities
        .iter()
        .filter(|(_, live)| live.components.contains_key(&ComponentKind::Camera))
        .map(|(id, _)| *id)
        .collect();
    for id in cameras {
        let camera = match world.live(id)?.components.get(&ComponentKind::Camera) {
            Some(Component::Camera(camera)) => *camera,
            _ => continue,
        };
        if let Some(target) = camera.follow.filter(|target| *target != id) {
            let target = world.transform(target)?;
            let k = (camera.lerp * dt).min(1.0);
            let own = transform_mut(world, id)?;
            own.x += (target.x - own.x) * k;
            own.y += (target.y - own.y) * k;
        }
        if let Some(bounds) = camera.bounds {
            let own = transform_mut(world, id)?;
            own.x = own.x.clamp(bounds.x, bounds.x + bounds.width);
            own.y = own.y.clamp(bounds.y, bounds.y + bounds.height);
        }
        let own = world.transform(id)?;
        if !(own.x.abs() <= MAX_COORD && own.y.abs() <= MAX_COORD) {
            return Err(GameError::Diverged);
        }
    }
    Ok(())
}

pub(crate) fn view(world: &World, camera: EntityId) -> Result<View, GameError> {
    let live = world.live(camera)?;
    let Some(Component::Camera(Camera {
        width,
        height,
        zoom,
        ..
    })) = live.components.get(&ComponentKind::Camera)
    else {
        return Err(GameError::NoSuchEntity(camera));
    };
    let center = world.transform(camera)?;
    Ok(View {
        center_x: center.x,
        center_y: center.y,
        zoom: *zoom,
        half_width: width / 2.0,
        half_height: height / 2.0,
    })
}

fn transform_mut(world: &mut World, id: EntityId) -> Result<&mut Transform, GameError> {
    let live = world
        .entities
        .get_mut(&id)
        .ok_or(GameError::NoSuchEntity(id))?;
    match live.components.get_mut(&ComponentKind::Transform) {
        Some(Component::Transform(t)) => Ok(t),
        _ => Err(GameError::NoSuchEntity(id)),
    }
}
