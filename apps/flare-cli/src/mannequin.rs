//! `flare mannequin`: the reference mannequin from the command line. What a figure is, its
//! picture and how it is placed are `flare_mannequin` and `flare_tools::mannequin`; this reads
//! and writes files and prints.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use flare_mannequin::{Look, MAX_FIGURE_BYTES, MAX_PICTURE_SIDE, Placement, Request};
use serde_json::{Value, json};

#[derive(Subcommand)]
pub enum MannequinCommand {
    /// List the bodies, built-in poses, proportions (with their ranges) and joints (with how far
    /// each turns, in degrees).
    Presets,
    /// Print a figure as JSON: the form `--figure` reads and the desktop panel keeps.
    Figure {
        #[command(flatten)]
        look: LookArgs,
    },
    /// Write a PNG of the figure shaded as a 3D drawing figure, with its guide lines, outline
    /// and shadow on the floor, see-through elsewhere.
    Picture {
        out: PathBuf,
        #[command(flatten)]
        look: LookArgs,
        /// The picture's size in pixels.
        #[arg(long, default_value_t = 600)]
        width: u32,
        #[arg(long, default_value_t = 800)]
        height: u32,
        /// Leave out the guide lines.
        #[arg(long)]
        no_guides: bool,
    },
    /// Place the figure, shaded as the panel shows it, as a picture on a new layer at the
    /// top of the first scene, from `--frame` to the end of the scene, drawn half see-through
    /// while editing, as one undo step.
    Place {
        path: PathBuf,
        #[command(flatten)]
        look: LookArgs,
        /// Where the figure's middle goes on the stage; the stage's middle by default.
        #[arg(long)]
        x: Option<f64>,
        #[arg(long)]
        y: Option<f64>,
        /// How tall the figure stands, in stage units; four fifths of the stage by default.
        #[arg(long)]
        height: Option<f64>,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        /// The new layer's name (and the picture's in the library).
        #[arg(long, default_value = "Mannequin")]
        name: String,
        /// An existing layer to place it on, by id (`flare info` lists them), instead of a new
        /// one: into its keyframe at `--frame`, beside what it holds, or a keyframe of its own
        /// there when it shows nothing at that frame.
        #[arg(long)]
        layer: Option<uuid::Uuid>,
        /// With `--layer`: a keyframe of its own from `--frame`, cutting short the one shown
        /// there, so each stretch of frames can hold another figure.
        #[arg(long, requires = "layer")]
        new_keyframe: bool,
    },
}

/// What the figure looks like, each laid over the one before.
#[derive(Args)]
pub struct LookArgs {
    /// A figure file, as `flare mannequin figure` prints it.
    #[arg(long)]
    figure: Option<PathBuf>,
    /// male or female.
    #[arg(long)]
    body: Option<String>,
    /// A built-in pose (see `flare mannequin presets`).
    #[arg(long)]
    pose: Option<String>,
    /// A pose file: {"joints": {"left_elbow": [bend, twist, side], ...}, "lift": [x, y, z]}.
    #[arg(long)]
    pose_file: Option<PathBuf>,
    /// One joint, as joint=bend,twist,side in degrees (left_knee=45,0,0); may be repeated.
    #[arg(long = "joint", value_parser = flare_mannequin::parse_joint_setting)]
    joints: Vec<(String, [f64; 3])>,
    /// A hand or a foot pulled to a point, as end=x,y,z: x to the figure's left, y up from the
    /// ground, z to its front, the figure one unit tall (left_wrist=0.2,0.7,0.15); the arm or
    /// leg bends to follow. May be repeated.
    #[arg(long = "reach", value_parser = flare_mannequin::parse_reach_setting, allow_hyphen_values = true)]
    reach: Vec<(String, [f64; 3])>,
    /// Degrees the figure is turned to its left.
    #[arg(long, allow_hyphen_values = true)]
    yaw: Option<f64>,
    /// Degrees the view looks down on it (negative looks up).
    #[arg(long, allow_hyphen_values = true)]
    pitch: Option<f64>,
    /// Field of view in degrees; 0 is flat.
    #[arg(long)]
    fov: Option<f64>,
}

fn read_small(path: &Path) -> Result<String> {
    let meta =
        std::fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    if meta.len() > MAX_FIGURE_BYTES as u64 {
        bail!("{} is too large for a mannequin file", path.display());
    }
    std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

impl LookArgs {
    fn build(self) -> Result<flare_mannequin::Mannequin> {
        Request {
            figure: self.figure.as_deref().map(read_small).transpose()?,
            body: self.body,
            preset: self.pose,
            pose: self.pose_file.as_deref().map(read_small).transpose()?,
            joints: self.joints,
            reach: self.reach,
            yaw: self.yaw,
            pitch: self.pitch,
            fov: self.fov,
        }
        .build()
        .map_err(|e| anyhow!(e))
    }
}

pub fn run(op: MannequinCommand) -> Result<Value> {
    match op {
        MannequinCommand::Presets => Ok(flare_mannequin::catalog()),
        MannequinCommand::Figure { look } => {
            let figure = look.build()?;
            Ok(serde_json::from_str(&figure.to_json())?)
        }
        MannequinCommand::Picture {
            out,
            look,
            width,
            height,
            no_guides,
        } => {
            let figure = look.build()?;
            let sides = 16..=MAX_PICTURE_SIDE;
            if !(sides.contains(&width) && sides.contains(&height)) {
                bail!("the picture's width and height are each 16 to {MAX_PICTURE_SIDE}");
            }
            let (w, h) = (f64::from(width), f64::from(height));
            let placement = Placement::new([w / 2.0, h / 2.0], (h * 0.9).min(w * 1.6))
                .context("the picture is too small")?;
            let look = Look {
                guides: !no_guides,
                ..Look::default()
            };
            let picture = figure
                .render(placement, width, height, look)
                .context("the picture is too large")?;
            let bytes =
                flare_runtime::rgba_png(width, height, picture.rgba()).map_err(|e| anyhow!(e))?;
            std::fs::write(&out, bytes)
                .with_context(|| format!("cannot write {}", out.display()))?;
            Ok(json!({ "path": out, "width": width, "height": height }))
        }
        MannequinCommand::Place {
            path,
            look,
            x,
            y,
            height,
            frame,
            name,
            layer,
            new_keyframe,
        } => {
            let figure = look.build()?;
            let mut session = crate::project::open_session(&path)?;
            let scene = session
                .document()
                .scenes()
                .first()
                .context("the document has no scene")?
                .id();
            let stage = session.document().stage();
            let (w, h) = (f64::from(stage.width()), f64::from(stage.height()));
            let height = height.unwrap_or(h * 0.8);
            let placement = Placement::new([x.unwrap_or(w / 2.0), y.unwrap_or(h / 2.0)], height)
                .with_context(|| {
                    format!(
                        "the figure needs a place that is a number and a height from above 0 to {}",
                        flare_mannequin::MAX_PLACEMENT_HEIGHT
                    )
                })?;
            let document = session.document();
            let placed = match layer {
                None => {
                    flare_tools::mannequin::place(document, scene, frame, &figure, placement, &name)
                }
                Some(id) => {
                    let layer = document.scenes()[0]
                        .timeline()
                        .layers()
                        .iter()
                        .find(|l| *l.id().as_uuid() == id)
                        .with_context(|| format!("no layer {id} in the first scene"))?
                        .id();
                    let keyframe = if new_keyframe {
                        flare_tools::mannequin::Keyframe::New
                    } else {
                        flare_tools::mannequin::Keyframe::Join
                    };
                    flare_tools::mannequin::place_onto(
                        document, scene, frame, &figure, placement, layer, keyframe, &name,
                    )
                }
            }
            .map_err(|e| anyhow!("cannot place the mannequin: {e}"))?;
            session.apply(placed.commands)?;
            crate::project::commit_session(&session, &path)?;
            Ok(json!({
                "layer": placed.layer.as_uuid(),
                "bitmap": { "width": placed.size.0, "height": placed.size.1 },
                "frame": frame,
                "keyframe": placed.at.start,
                "element": placed.at.element,
            }))
        }
    }
}
