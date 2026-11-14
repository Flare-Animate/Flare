//! The range checks for each component. A component that passes is one a system can step
//! without producing a non-finite value.

use crate::limits::{
    MAX_ACTION_CHARS, MAX_ACTIONS, MAX_COORD, MAX_EMIT_RATE, MAX_FPS, MAX_NAME_CHARS, MAX_SIZE,
    MAX_SPEED, MAX_SPRITE_FRAMES, MAX_TEXT_CHARS,
};
use crate::{Component, DestroyWho, GameError, HookWhen};

pub(super) fn validate(component: &Component) -> Result<(), GameError> {
    let name = component.kind().name();
    let check = Check { component: name };
    match component {
        Component::Transform(t) => {
            check.coord("x", t.x)?;
            check.coord("y", t.y)?;
            check.finite("rotation", t.rotation)?;
            check.range("scale_x", t.scale_x, -MAX_SIZE, MAX_SIZE)?;
            check.range("scale_y", t.scale_y, -MAX_SIZE, MAX_SIZE)?;
            check.range("pivot_x", t.pivot_x, 0.0, 1.0)?;
            check.range("pivot_y", t.pivot_y, 0.0, 1.0)?;
        }
        Component::Sprite(s) => {
            check.range(
                "frames",
                f64::from(s.frames),
                1.0,
                f64::from(MAX_SPRITE_FRAMES),
            )?;
            check.range("fps", s.fps, 0.0, MAX_FPS)?;
            if s.frame >= s.frames {
                return Err(check.bad("frame", "must be below the number of frames"));
            }
            check.range("alpha", s.alpha, 0.0, 1.0)?;
        }
        Component::Body(b) => {
            check.range("width", b.width, 0.0, MAX_SIZE)?;
            check.range("height", b.height, 0.0, MAX_SIZE)?;
            check.range("vx", b.vx, -MAX_SPEED, MAX_SPEED)?;
            check.range("vy", b.vy, -MAX_SPEED, MAX_SPEED)?;
        }
        Component::Control(c) => {
            check.range("speed", c.speed, 0.0, MAX_SPEED)?;
        }
        Component::Camera(c) => {
            check.range("width", c.width, 1.0, MAX_SIZE)?;
            check.range("height", c.height, 1.0, MAX_SIZE)?;
            check.range("zoom", c.zoom, 0.01, 100.0)?;
            check.range("lerp", c.lerp, 0.0, 1000.0)?;
            if let Some(b) = c.bounds {
                check.coord("bounds_x", b.x)?;
                check.coord("bounds_y", b.y)?;
                check.range("bounds_width", b.width, 0.0, MAX_SIZE)?;
                check.range("bounds_height", b.height, 0.0, MAX_SIZE)?;
            }
        }
        Component::Ui(u) => {
            check.range("width", u.width, 0.0, MAX_SIZE)?;
            check.range("height", u.height, 0.0, MAX_SIZE)?;
            if u.text.chars().count() > MAX_TEXT_CHARS {
                return Err(check.bad(
                    "text",
                    &format!("is longer than {MAX_TEXT_CHARS} characters"),
                ));
            }
        }
        Component::Audio(a) => {
            check.range("volume", a.volume, 0.0, 1.0)?;
        }
        Component::Particle(p) => {
            check.range("rate", p.rate, 0.0, MAX_EMIT_RATE)?;
            check.range("life", p.life, 0.0, 3600.0)?;
        }
        Component::Script(s) => {
            let len = s.name.chars().count();
            if len == 0 || len > MAX_NAME_CHARS || s.name.chars().any(char::is_control) {
                return Err(check.bad(
                    "name",
                    "must be 1 to 128 characters with no control characters",
                ));
            }
        }
        Component::Trigger(t) => {
            check.range("width", t.width, 0.0, MAX_SIZE)?;
            check.range("height", t.height, 0.0, MAX_SIZE)?;
        }
        Component::Hook(h) => {
            if let HookWhen::Action(name) = &h.when {
                check_action_name(&check, "when", name)?;
            }
            if h.follow && !matches!(h.when, HookWhen::Trigger) {
                return Err(check.bad("follow", "needs a trigger to have a visitor to follow"));
            }
            if h.destroy == DestroyWho::Visitor && !matches!(h.when, HookWhen::Trigger) {
                return Err(check.bad(
                    "destroy",
                    "visitor needs a trigger to have a visitor at all",
                ));
            }
        }
        Component::Actions(a) => {
            if a.names.len() > MAX_ACTIONS {
                return Err(check.bad("names", &format!("names more than {MAX_ACTIONS} actions")));
            }
            let mut seen: Vec<&str> = Vec::new();
            for name in &a.names {
                check_action_name(&check, "names", name)?;
                if seen.contains(&name.as_str()) {
                    return Err(check.bad("names", &format!("names {name:?} twice")));
                }
                seen.push(name);
            }
        }
    }
    Ok(())
}

fn check_action_name(check: &Check, field: &'static str, name: &str) -> Result<(), GameError> {
    let len = name.chars().count();
    if len == 0 || len > MAX_ACTION_CHARS || name.chars().any(char::is_control) {
        return Err(check.bad(
            field,
            &format!("must be 1 to {MAX_ACTION_CHARS} characters with no control characters"),
        ));
    }
    Ok(())
}

/// The checks shared by every component: finite numbers, and ranges.
pub(super) struct Check {
    pub(super) component: &'static str,
}

impl Check {
    pub(super) fn bad(&self, field: &'static str, reason: &str) -> GameError {
        GameError::BadField {
            component: self.component,
            field,
            reason: reason.to_owned(),
        }
    }

    pub(super) fn finite(&self, field: &'static str, value: f64) -> Result<(), GameError> {
        if value.is_finite() {
            Ok(())
        } else {
            Err(self.bad(field, "must be a finite number"))
        }
    }

    pub(super) fn range(
        &self,
        field: &'static str,
        value: f64,
        min: f64,
        max: f64,
    ) -> Result<(), GameError> {
        self.finite(field, value)?;
        if value < min || value > max {
            return Err(self.bad(field, &format!("must be from {min} to {max}")));
        }
        Ok(())
    }

    pub(super) fn coord(&self, field: &'static str, value: f64) -> Result<(), GameError> {
        self.range(field, value, -MAX_COORD, MAX_COORD)
    }
}
