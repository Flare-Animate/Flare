//! Reading a component from `name=value` fields, as the command line and MCP send them.
//! Every field is named and checked; a misspelt field is an error, not a default.

use std::collections::BTreeMap;

use flare_core::SoundId;
use uuid::Uuid;

use crate::{
    Actions, Audio, Body, BodyKind, Bounds, Camera, Component, ComponentKind, Control, DestroyWho,
    EntityId, GameError, Hook, HookWhen, Key, Particle, Script, Sprite, Transform, Trigger, Ui,
    UiKind,
};

pub(super) fn parse(kind: &str, fields: &[(String, String)]) -> Result<Component, GameError> {
    let kind =
        ComponentKind::parse(kind).ok_or_else(|| GameError::UnknownComponent(kind.to_owned()))?;
    let mut fields = Fields::new(kind, fields)?;
    let component = match kind {
        ComponentKind::Transform => {
            let d = Transform::default();
            Component::Transform(Transform {
                x: fields.f64("x", d.x)?,
                y: fields.f64("y", d.y)?,
                rotation: fields.f64("rotation", d.rotation)?,
                scale_x: fields.f64("scale_x", d.scale_x)?,
                scale_y: fields.f64("scale_y", d.scale_y)?,
                pivot_x: fields.f64("pivot_x", d.pivot_x)?,
                pivot_y: fields.f64("pivot_y", d.pivot_y)?,
            })
        }
        ComponentKind::Sprite => {
            let d = Sprite::default();
            Component::Sprite(Sprite {
                frames: fields.u32("frames", d.frames)?,
                fps: fields.f64("fps", d.fps)?,
                frame: fields.u32("frame", d.frame)?,
                looping: fields.bool("loop", d.looping)?,
                visible: fields.bool("visible", d.visible)?,
                alpha: fields.f64("alpha", d.alpha)?,
                order: fields.i32("order", d.order)?,
            })
        }
        ComponentKind::Body => {
            let d = Body::default();
            let kind = match fields.text("kind") {
                None => d.kind,
                Some(text) => match text.as_str() {
                    "static" => BodyKind::Static,
                    "kinematic" => BodyKind::Kinematic,
                    "dynamic" => BodyKind::Dynamic,
                    _ => return Err(fields.bad("kind", "must be static, kinematic or dynamic")),
                },
            };
            Component::Body(Body {
                kind,
                width: fields.f64("width", d.width)?,
                height: fields.f64("height", d.height)?,
                vx: fields.f64("vx", d.vx)?,
                vy: fields.f64("vy", d.vy)?,
                category: fields.u16("category", d.category)?,
                mask: fields.u16("mask", d.mask)?,
            })
        }
        ComponentKind::Control => {
            let d = Control::default();
            Component::Control(Control {
                left: fields.key("left", d.left)?,
                right: fields.key("right", d.right)?,
                up: fields.key("up", d.up)?,
                down: fields.key("down", d.down)?,
                speed: fields.f64("speed", d.speed)?,
            })
        }
        ComponentKind::Camera => {
            let d = Camera::default();
            let bounds = match fields.text("bounds") {
                None => None,
                Some(text) => Some(
                    parse_bounds(&text)
                        .ok_or_else(|| fields.bad("bounds", "must be x,y,width,height"))?,
                ),
            };
            Component::Camera(Camera {
                width: fields.f64("width", d.width)?,
                height: fields.f64("height", d.height)?,
                zoom: fields.f64("zoom", d.zoom)?,
                follow: match fields.u32_opt("follow")? {
                    None | Some(0) => None,
                    Some(raw) => Some(
                        EntityId::new(raw)
                            .ok_or_else(|| fields.bad("follow", "is not an entity"))?,
                    ),
                },
                lerp: fields.f64("lerp", d.lerp)?,
                bounds,
            })
        }
        ComponentKind::Ui => {
            let d = Ui::default();
            let kind = match fields.text("kind") {
                None => d.kind,
                Some(text) => {
                    UiKind::parse(&text).ok_or_else(|| fields.bad("kind", "is not a UI kind"))?
                }
            };
            Component::Ui(Ui {
                kind,
                width: fields.f64("width", d.width)?,
                height: fields.f64("height", d.height)?,
                text: fields.text("text").unwrap_or(d.text),
                interactive: fields.bool("interactive", d.interactive)?,
            })
        }
        ComponentKind::Audio => {
            let d = Audio::default();
            let sound = match fields.text("sound") {
                None => None,
                Some(text) => Some(SoundId::from_uuid(
                    Uuid::parse_str(&text).map_err(|_| fields.bad("sound", "is not a sound ID"))?,
                )),
            };
            Component::Audio(Audio {
                sound,
                looping: fields.bool("loop", d.looping)?,
                play_on_start: fields.bool("play_on_start", d.play_on_start)?,
                volume: fields.f64("volume", d.volume)?,
            })
        }
        ComponentKind::Particle => {
            let d = Particle::default();
            Component::Particle(Particle {
                rate: fields.f64("rate", d.rate)?,
                life: fields.f64("life", d.life)?,
                auto_play: fields.bool("auto_play", d.auto_play)?,
            })
        }
        ComponentKind::Script => {
            let name = fields
                .text("name")
                .ok_or_else(|| fields.bad("name", "is required"))?;
            Component::Script(Script { name })
        }
        ComponentKind::Trigger => {
            let d = Trigger::default();
            Component::Trigger(Trigger {
                width: fields.f64("width", d.width)?,
                height: fields.f64("height", d.height)?,
                once: fields.bool("once", d.once)?,
            })
        }
        ComponentKind::Hook => {
            let d = Hook::default();
            let when = match fields.text("when") {
                None => d.when,
                Some(text) => {
                    if text == "trigger" {
                        HookWhen::Trigger
                    } else if let Some(name) = text.strip_prefix("action:") {
                        HookWhen::Action(name.trim().to_owned())
                    } else {
                        return Err(fields.bad("when", "must be trigger or action:<name>"));
                    }
                }
            };
            let sound = match fields.text("sound") {
                None => None,
                Some(text) => Some(SoundId::from_uuid(
                    Uuid::parse_str(&text).map_err(|_| fields.bad("sound", "is not a sound ID"))?,
                )),
            };
            let spawn = match fields.u32_opt("spawn")? {
                None | Some(0) => None,
                Some(raw) => Some(
                    EntityId::new(raw).ok_or_else(|| fields.bad("spawn", "is not an entity"))?,
                ),
            };
            Component::Hook(Hook {
                when,
                sound,
                spawn,
                follow: fields.bool("follow", d.follow)?,
                destroy: match fields.text("destroy") {
                    None => d.destroy,
                    Some(text) => DestroyWho::parse(&text)
                        .ok_or_else(|| fields.bad("destroy", "must be never, visitor or this"))?,
                },
            })
        }
        ComponentKind::Actions => {
            let names = match fields.text("names") {
                None => Vec::new(),
                Some(text) => text
                    .split(',')
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty())
                    .collect(),
            };
            Component::Actions(Actions { names })
        }
    };
    fields.finish()?;
    component.validate()?;
    Ok(component)
}

/// `x,y,width,height`.
fn parse_bounds(text: &str) -> Option<Bounds> {
    let parts: Vec<f64> = text
        .split(',')
        .map(|part| part.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    match parts.as_slice() {
        [x, y, width, height] => Some(Bounds {
            x: *x,
            y: *y,
            width: *width,
            height: *height,
        }),
        _ => None,
    }
}

/// Most fields one component may be given.
const MAX_FIELDS: usize = 32;

/// The fields of one component, taken one by one. Whatever is left when the component is built
/// is a field the component does not have.
struct Fields {
    component: &'static str,
    values: BTreeMap<String, String>,
}

impl Fields {
    fn new(kind: ComponentKind, pairs: &[(String, String)]) -> Result<Self, GameError> {
        if pairs.len() > MAX_FIELDS {
            return Err(GameError::BadInput(
                "too many fields for one component".to_owned(),
            ));
        }
        let mut values = BTreeMap::new();
        for (name, value) in pairs {
            if values.insert(name.clone(), value.clone()).is_some() {
                return Err(GameError::BadInput(format!(
                    "field {name:?} is given twice"
                )));
            }
        }
        Ok(Self {
            component: kind.name(),
            values,
        })
    }

    fn bad(&self, field: &'static str, reason: &str) -> GameError {
        GameError::BadField {
            component: self.component,
            field,
            reason: reason.to_owned(),
        }
    }

    fn text(&mut self, name: &str) -> Option<String> {
        self.values.remove(name)
    }

    fn f64(&mut self, name: &'static str, default: f64) -> Result<f64, GameError> {
        match self.values.remove(name) {
            None => Ok(default),
            Some(text) => text
                .trim()
                .parse::<f64>()
                .map_err(|_| self.bad(name, "is not a number")),
        }
    }

    fn u32(&mut self, name: &'static str, default: u32) -> Result<u32, GameError> {
        Ok(self.u32_opt(name)?.unwrap_or(default))
    }

    fn u32_opt(&mut self, name: &'static str) -> Result<Option<u32>, GameError> {
        match self.values.remove(name) {
            None => Ok(None),
            Some(text) => text
                .trim()
                .parse::<u32>()
                .map(Some)
                .map_err(|_| self.bad(name, "is not a whole number")),
        }
    }

    fn u16(&mut self, name: &'static str, default: u16) -> Result<u16, GameError> {
        match self.values.remove(name) {
            None => Ok(default),
            Some(text) => text
                .trim()
                .parse::<u16>()
                .map_err(|_| self.bad(name, "is not a number from 0 to 65535")),
        }
    }

    fn i32(&mut self, name: &'static str, default: i32) -> Result<i32, GameError> {
        match self.values.remove(name) {
            None => Ok(default),
            Some(text) => text
                .trim()
                .parse::<i32>()
                .map_err(|_| self.bad(name, "is not a whole number")),
        }
    }

    fn bool(&mut self, name: &'static str, default: bool) -> Result<bool, GameError> {
        match self.values.remove(name).as_deref() {
            None => Ok(default),
            Some("true") | Some("1") | Some("yes") => Ok(true),
            Some("false") | Some("0") | Some("no") => Ok(false),
            Some(_) => Err(self.bad(name, "must be true or false")),
        }
    }

    fn key(&mut self, name: &'static str, default: Key) -> Result<Key, GameError> {
        match self.values.remove(name) {
            None => Ok(default),
            Some(text) => Key::parse(&text).ok_or(GameError::UnknownKey(text)),
        }
    }

    fn finish(self) -> Result<(), GameError> {
        match self.values.into_keys().next() {
            None => Ok(()),
            Some(field) => Err(GameError::UnknownField {
                component: self.component,
                field,
            }),
        }
    }
}
