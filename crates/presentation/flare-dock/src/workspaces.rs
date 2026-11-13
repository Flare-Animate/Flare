//! Named workspaces, as Animate has them: a few built in, and the person's own saved under a name.
//! Each keeps the changes made to it while it was in use, and Reset takes it back to how it was
//! built in or saved.

use serde::{Deserialize, Serialize};

use crate::arrange::{self, Plan};
use crate::{Layout, LayoutError, Pane, layout};

/// Largest workspaces file accepted: it holds several layouts.
pub const MAX_WORKSPACES_BYTES: usize = 1024 * 1024;
/// How many workspaces of their own a person can save.
pub const MAX_CUSTOM: usize = 20;
/// Longest name of a saved workspace, in characters.
pub const MAX_NAME_CHARS: usize = 40;

/// The built-in workspaces, modelled on Animate's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Properties, Library and Assets on the right, the rest in the strip, the timeline below.
    Essentials,
    /// The timeline above the stage and two stacks on the right, for keying motion.
    Animator,
    /// Properties at the left of the stage and the drawing panels open on the right.
    Designer,
    /// The timeline above the stage and most panels in the strip, as Flash had them.
    Classic,
    /// Every panel in one stack on the right and nothing in the strip, for a small display.
    SmallScreen,
}

impl Preset {
    pub const ALL: [Self; 5] = [
        Self::Essentials,
        Self::Animator,
        Self::Designer,
        Self::Classic,
        Self::SmallScreen,
    ];

    /// The key of the workspace's name in the interface's language.
    pub fn key(self) -> &'static str {
        match self {
            Self::Essentials => "workspace-essentials",
            Self::Animator => "workspace-animator",
            Self::Designer => "workspace-designer",
            Self::Classic => "workspace-classic",
            Self::SmallScreen => "workspace-small-screen",
        }
    }

    /// The workspace's name in English, for the command line and messages.
    pub fn title(self) -> &'static str {
        match self {
            Self::Essentials => "Essentials",
            Self::Animator => "Animator",
            Self::Designer => "Designer",
            Self::Classic => "Classic",
            Self::SmallScreen => "Small Screen",
        }
    }

    /// The arrangement the workspace starts with.
    pub fn layout(self) -> Layout {
        use Pane as P;
        let tree = match self {
            Self::Essentials => return layout::default_layout(),
            Self::Animator => arrange::arrange(&Plan {
                timeline_on_top: true,
                left: &[],
                right: &[
                    &[P::Properties, P::Library, P::Assets],
                    &[P::Color, P::Swatches, P::Align, P::Transform, P::Info],
                ],
            }),
            Self::Designer => arrange::arrange(&Plan {
                timeline_on_top: true,
                left: &[&[P::Properties]],
                right: &[
                    &[P::Library, P::Assets],
                    &[P::Align, P::Info, P::Transform],
                    &[P::Color, P::Swatches],
                ],
            }),
            Self::Classic => arrange::arrange(&Plan {
                timeline_on_top: true,
                left: &[],
                right: &[&[P::Properties, P::Assets]],
            }),
            Self::SmallScreen => arrange::arrange(&Plan {
                timeline_on_top: false,
                left: &[],
                right: &[&[
                    P::Properties,
                    P::Library,
                    P::Assets,
                    P::Layers,
                    P::Color,
                    P::Swatches,
                    P::Align,
                    P::Transform,
                    P::Info,
                    P::History,
                    P::MotionEditor,
                    P::FindReplace,
                    P::Mannequin,
                ]],
            }),
        };
        let strip = Pane::ALL
            .into_iter()
            .filter(|p| p.can_collapse() && arrange::find(&tree, *p).is_none())
            .collect();
        Layout { tree, strip }
    }
}

/// Which workspace: a built-in one, or one the person saved under a name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceId {
    Preset(Preset),
    Custom(String),
}

/// A workspace the person saved: its name and the arrangement Reset returns to.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Custom {
    pub name: String,
    pub layout: Layout,
}

/// The workspaces and which one is in use. The arrangement on screen is kept by the caller and
/// handed in when it is put away, so this holds every workspace but the one showing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspaces {
    pub active: WorkspaceId,
    /// Each workspace as it was last left, where it differs from how it was built in or saved.
    #[serde(default)]
    live: Vec<(WorkspaceId, Layout)>,
    #[serde(default)]
    custom: Vec<Custom>,
}

impl Default for Workspaces {
    fn default() -> Self {
        Self {
            active: WorkspaceId::Preset(Preset::Essentials),
            live: Vec::new(),
            custom: Vec::new(),
        }
    }
}

impl Workspaces {
    /// The names of the person's own workspaces, in the order they were saved.
    pub fn custom_names(&self) -> impl Iterator<Item = &str> {
        self.custom.iter().map(|c| c.name.as_str())
    }

    /// How `id` was built in or saved; none for a name that was never saved.
    fn pristine(&self, id: &WorkspaceId) -> Option<Layout> {
        match id {
            WorkspaceId::Preset(preset) => Some(preset.layout()),
            WorkspaceId::Custom(name) => self
                .custom
                .iter()
                .find(|c| &c.name == name)
                .map(|c| c.layout.clone()),
        }
    }

    fn keep(&mut self, id: WorkspaceId, layout: Layout) {
        self.live.retain(|(kept, _)| *kept != id);
        self.live.push((id, layout));
    }

    /// Puts `current` away as the active workspace's arrangement and makes `to` active, returning
    /// its arrangement as it was last left. A name that was never saved is refused and nothing
    /// changes.
    pub fn switch(&mut self, current: Layout, to: WorkspaceId) -> Result<Layout, LayoutError> {
        let pristine = self.pristine(&to).ok_or_else(|| unknown(&to))?;
        let next = self
            .live
            .iter()
            .find(|(id, _)| *id == to)
            .map_or(pristine, |(_, layout)| layout.clone());
        let from = std::mem::replace(&mut self.active, to.clone());
        self.keep(from, current);
        self.live.retain(|(id, _)| *id != to);
        Ok(next)
    }

    /// The active workspace as it was built in or saved; its changes are forgotten.
    pub fn reset(&mut self) -> Layout {
        let active = self.active.clone();
        self.live.retain(|(id, _)| *id != active);
        self.pristine(&active)
            .unwrap_or_else(|| Preset::Essentials.layout())
    }

    /// Saves `current` as a workspace of the person's own named `name` and makes it the active one.
    /// A name already saved (ignoring case) is replaced. An empty or too long name, or one more
    /// than [`MAX_CUSTOM`], is refused and nothing changes.
    pub fn save_as(&mut self, name: &str, current: Layout) -> Result<(), LayoutError> {
        let name = valid_name(name)?;
        let existing = self
            .custom
            .iter()
            .position(|c| c.name.to_lowercase() == name.to_lowercase());
        if existing.is_none() && self.custom.len() >= MAX_CUSTOM {
            return Err(LayoutError::Invalid(format!(
                "at most {MAX_CUSTOM} workspaces can be saved"
            )));
        }
        let saved = Custom {
            name: name.clone(),
            layout: current,
        };
        if let Some(at) = existing {
            let old = std::mem::replace(&mut self.custom[at], saved);
            let old = WorkspaceId::Custom(old.name);
            self.live.retain(|(id, _)| *id != old);
        } else {
            self.custom.push(saved);
        }
        // The arrangement left in the workspace in use is kept for when it is chosen again.
        let id = WorkspaceId::Custom(name);
        if self.active != id {
            let from = std::mem::replace(&mut self.active, id.clone());
            if let Some(layout) = self.pristine(&id) {
                self.keep(from, layout);
            }
        }
        self.live.retain(|(kept, _)| *kept != id);
        Ok(())
    }

    /// Forgets a saved workspace. When it is the one in use, Essentials takes its place and its
    /// arrangement is returned for the caller to show.
    pub fn delete(&mut self, name: &str) -> Result<Option<Layout>, LayoutError> {
        let id = WorkspaceId::Custom(name.to_owned());
        let at = self
            .custom
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| unknown(&id))?;
        self.custom.remove(at);
        self.live.retain(|(kept, _)| *kept != id);
        if self.active != id {
            return Ok(None);
        }
        self.active = WorkspaceId::Preset(Preset::Essentials);
        let layout = self
            .live
            .iter()
            .position(|(kept, _)| *kept == self.active)
            .map(|at| self.live.remove(at).1)
            .unwrap_or_else(|| Preset::Essentials.layout());
        Ok(Some(layout))
    }

    fn check(&self) -> Result<(), LayoutError> {
        if self.custom.len() > MAX_CUSTOM {
            return Err(LayoutError::Invalid(format!(
                "more than {MAX_CUSTOM} saved workspaces"
            )));
        }
        for (at, custom) in self.custom.iter().enumerate() {
            valid_name(&custom.name)?;
            layout::check(&custom.layout)?;
            if self.custom[..at]
                .iter()
                .any(|c| c.name.to_lowercase() == custom.name.to_lowercase())
            {
                return Err(LayoutError::Invalid(format!(
                    "the workspace {} is saved twice",
                    custom.name
                )));
            }
        }
        if self.live.len() > Preset::ALL.len() + self.custom.len() {
            return Err(LayoutError::Invalid("too many kept arrangements".into()));
        }
        for (id, kept) in &self.live {
            self.pristine(id).ok_or_else(|| unknown(id))?;
            layout::check(kept)?;
        }
        self.pristine(&self.active)
            .ok_or_else(|| unknown(&self.active))?;
        Ok(())
    }
}

fn unknown(id: &WorkspaceId) -> LayoutError {
    match id {
        WorkspaceId::Custom(name) => LayoutError::Invalid(format!("no workspace is named {name}")),
        WorkspaceId::Preset(preset) => {
            LayoutError::Invalid(format!("the {} workspace is unknown", preset.title()))
        }
    }
}

fn valid_name(name: &str) -> Result<String, LayoutError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LayoutError::Invalid("a workspace needs a name".into()));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(LayoutError::Invalid(format!(
            "a workspace name has at most {MAX_NAME_CHARS} characters"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(LayoutError::Invalid(
            "a workspace name cannot hold control characters".into(),
        ));
    }
    Ok(name.to_owned())
}

/// The workspaces with `current` as the arrangement of the active one, for saving.
pub fn workspaces_to_json(
    workspaces: &Workspaces,
    current: &Layout,
) -> Result<String, serde_json::Error> {
    let mut all = workspaces.clone();
    let active = all.active.clone();
    all.keep(active, current.clone());
    serde_json::to_string(&all)
}

/// Reads saved workspaces, returning them and the active one's arrangement. A file from before
/// workspaces (a layout, or a bare tree) opens as Essentials arranged as it says.
pub fn workspaces_from_json(json: &str) -> Result<(Workspaces, Layout), LayoutError> {
    if json.len() > MAX_WORKSPACES_BYTES {
        return Err(LayoutError::TooLarge);
    }
    let value: serde_json::Value = serde_json::from_str(json)?;
    if value.get("active").is_none() {
        let layout = layout::from_json(json)?;
        return Ok((Workspaces::default(), layout));
    }
    let mut workspaces: Workspaces = serde_json::from_value(value)?;
    for layout in workspaces
        .live
        .iter_mut()
        .map(|(_, layout)| layout)
        .chain(workspaces.custom.iter_mut().map(|c| &mut c.layout))
    {
        layout.add_missing_to_strip();
    }
    workspaces.check()?;
    let active = workspaces.active.clone();
    let current = match workspaces.live.iter().position(|(id, _)| *id == active) {
        Some(at) => workspaces.live.remove(at).1,
        None => workspaces
            .pristine(&active)
            .ok_or_else(|| unknown(&active))?,
    };
    Ok((workspaces, current))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "workspaces_tests.rs"]
mod tests;
