//! The physics system: the controls set velocities, gravity pulls dynamic bodies, bodies move,
//! and overlapping bodies are reported as collisions. The C++ scaffold called Box2D here and
//! never connected it, so this is the port's own small model. Nothing is pushed apart: a
//! collision is an event, not a contact.

use std::collections::BTreeSet;

use crate::limits::MAX_COORD;
use crate::system::StepContext;
use crate::{BodyKind, Component, ComponentKind, EntityId, Event, GameError, World};

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    let dt = ctx.dt;
    let (gx, gy) = world.gravity;
    let input = ctx.input;

    for live in world.entities.values_mut() {
        let control = match live.components.get(&ComponentKind::Control) {
            Some(Component::Control(control)) => *control,
            _ => continue,
        };
        if let Some(Component::Body(body)) = live.components.get_mut(&ComponentKind::Body) {
            if body.kind == BodyKind::Static {
                continue;
            }
            let h = axis(input.key_down(control.right), input.key_down(control.left));
            let v = axis(input.key_down(control.down), input.key_down(control.up));
            body.vx = h * control.speed;
            body.vy = v * control.speed;
        }
    }

    for live in world.entities.values_mut() {
        let body = match live.components.get(&ComponentKind::Body) {
            Some(Component::Body(body)) => *body,
            _ => continue,
        };
        if body.kind == BodyKind::Static {
            continue;
        }
        let (mut vx, mut vy) = (body.vx, body.vy);
        if body.kind == BodyKind::Dynamic {
            vx += gx * dt;
            vy += gy * dt;
        }
        let Some(Component::Transform(t)) = live.components.get_mut(&ComponentKind::Transform)
        else {
            continue;
        };
        t.x += vx * dt;
        t.y += vy * dt;
        if !(t.x.abs() <= MAX_COORD && t.y.abs() <= MAX_COORD) {
            return Err(GameError::Diverged);
        }
        if let Some(Component::Body(stored)) = live.components.get_mut(&ComponentKind::Body) {
            stored.vx = vx;
            stored.vy = vy;
        }
    }

    let now = overlapping(world);
    for pair in now.difference(&world.overlaps) {
        ctx.emit(Event::Collision {
            a: pair.0,
            b: pair.1,
            begin: true,
        })?;
    }
    for pair in world.overlaps.difference(&now) {
        ctx.emit(Event::Collision {
            a: pair.0,
            b: pair.1,
            begin: false,
        })?;
    }
    world.overlaps = now;
    Ok(())
}

fn axis(positive: bool, negative: bool) -> f64 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

/// The pairs of bodies that overlap now and whose categories and masks let them meet. Two static
/// bodies never meet.
fn overlapping(world: &World) -> BTreeSet<(EntityId, EntityId)> {
    struct Box2 {
        id: EntityId,
        min: (f64, f64),
        max: (f64, f64),
        category: u16,
        mask: u16,
        fixed: bool,
    }
    let boxes: Vec<Box2> = world
        .entities
        .iter()
        .filter_map(|(id, live)| {
            let Component::Body(body) = live.components.get(&ComponentKind::Body)? else {
                return None;
            };
            let Component::Transform(t) = live.components.get(&ComponentKind::Transform)? else {
                return None;
            };
            Some(Box2 {
                id: *id,
                min: (t.x, t.y),
                max: (t.x + body.width, t.y + body.height),
                category: body.category,
                mask: body.mask,
                fixed: body.kind == BodyKind::Static,
            })
        })
        .collect();
    let mut out = BTreeSet::new();
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            if a.fixed && b.fixed {
                continue;
            }
            if a.category & b.mask == 0 || b.category & a.mask == 0 {
                continue;
            }
            if a.min.0 < b.max.0 && b.min.0 < a.max.0 && a.min.1 < b.max.1 && b.min.1 < a.max.1 {
                out.insert((a.id, b.id));
            }
        }
    }
    out
}
