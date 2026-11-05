//! A fingerprint of a world's state. Two runs of the same description and the same script give
//! the same digest; a test and the surfaces compare runs with it.

use crate::{Component, ComponentKind, World};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

impl World {
    /// A 64-bit FNV-1a hash of every entity's state, in ID order. Floats are hashed by their bits,
    /// so the digest changes with the smallest difference in any value.
    pub fn digest(&self) -> u64 {
        let mut hash = FNV_OFFSET;
        mix(&mut hash, &self.steps().to_le_bytes());
        for (id, live) in &self.entities {
            mix(&mut hash, &id.get().to_le_bytes());
            mix(&mut hash, &live.frame.to_le_bytes());
            mix(&mut hash, &live.clock.to_bits().to_le_bytes());
            mix(&mut hash, &live.emit_carry.to_bits().to_le_bytes());
            mix(&mut hash, &[u8::from(live.sounded)]);
            for kind in ComponentKind::ALL {
                match live.components.get(&kind) {
                    None => mix(&mut hash, &[0]),
                    Some(component) => {
                        mix(&mut hash, &[1]);
                        for value in numbers(component) {
                            mix(&mut hash, &value.to_bits().to_le_bytes());
                        }
                    }
                }
            }
        }
        hash
    }
}

fn mix(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

/// The numeric state of a component, in a fixed order. Non-numeric fields are not part of the
/// state the systems change, so they are left out.
fn numbers(component: &Component) -> Vec<f64> {
    match component {
        Component::Transform(t) => vec![t.x, t.y, t.rotation, t.scale_x, t.scale_y],
        Component::Sprite(s) => vec![s.fps, s.alpha, f64::from(s.frames)],
        Component::Body(b) => vec![b.width, b.height, b.vx, b.vy],
        Component::Control(c) => vec![c.speed],
        Component::Camera(c) => vec![c.width, c.height, c.zoom, c.lerp],
        Component::Ui(u) => vec![u.width, u.height],
        Component::Audio(a) => vec![a.volume],
        Component::Particle(p) => vec![p.rate, p.life],
        Component::Script(_) => Vec::new(),
    }
}
