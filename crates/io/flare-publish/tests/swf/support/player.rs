//! A small SWF player for the tags Flare writes: display lists, sprites with a playhead of their
//! own, `stop()`, and clip layers. It plays what the reader decoded, so what a test checks is
//! the file and not the publisher's idea of it.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, HashMap};

use flare_core::Rgba8;
use flare_document::Shape;
use glamx::DAffine2;
use swf::{PlaceObjectAction, Tag};

use super::{Parsed, rebuild, to_affine};

/// What Flare writes for `stop()`: the action and the end of the list.
const STOP: &[u8] = &[0x07, 0x00];

#[derive(Clone)]
struct Object {
    character: u16,
    matrix: DAffine2,
    clip_depth: Option<u16>,
    /// Which frame of its own timeline a sprite is on.
    frame: usize,
    stopped: bool,
    children: BTreeMap<u16, Object>,
}

/// Frames of tags, each without its `ShowFrame`.
type Frames = Vec<Vec<Tag<'static>>>;

pub struct Player {
    sprites: HashMap<u16, Frames>,
    main: Frames,
    root: BTreeMap<u16, Object>,
    ticks: usize,
}

/// What is on screen after a frame: shapes bottom first with their place on the stage, and the
/// clip regions some of them lie inside.
#[derive(Default, Clone)]
pub struct Flat {
    pub shapes: Vec<FlatShape>,
    pub clips: Vec<Vec<(u16, DAffine2)>>,
}

#[derive(Clone)]
pub struct FlatShape {
    pub character: u16,
    pub transform: DAffine2,
    /// Indices into `Flat::clips`, outermost first.
    pub clips: Vec<usize>,
}

fn split(tags: &[Tag<'static>]) -> Frames {
    let mut frames = vec![Vec::new()];
    for tag in tags {
        if matches!(tag, Tag::ShowFrame) {
            frames.push(Vec::new());
        } else {
            let last = frames.len() - 1;
            frames[last].push(clone_tag(tag));
        }
    }
    // The tags after the last ShowFrame are not a frame.
    frames.pop();
    frames
}

/// The few tags a display list is made of, copied.
fn clone_tag(tag: &Tag<'static>) -> Tag<'static> {
    match tag {
        Tag::PlaceObject(p) => Tag::PlaceObject(Box::new(swf::PlaceObject {
            version: p.version,
            action: p.action,
            depth: p.depth,
            matrix: p.matrix,
            color_transform: None,
            ratio: None,
            name: None,
            clip_depth: p.clip_depth,
            class_name: None,
            filters: None,
            background_color: None,
            blend_mode: None,
            clip_actions: None,
            has_image: false,
            is_bitmap_cached: None,
            is_visible: None,
            amf_data: None,
        })),
        Tag::RemoveObject(r) => Tag::RemoveObject(swf::RemoveObject {
            depth: r.depth,
            character_id: r.character_id,
        }),
        Tag::DoAction(bytes) => Tag::DoAction(bytes),
        Tag::StartSound(_) => Tag::ShowFrame, // a sound is not part of a picture
        other => panic!("the player does not know this tag: {other:?}"),
    }
}

impl Player {
    pub fn new(parsed: &Parsed) -> Self {
        let mut sprites = HashMap::new();
        let mut main = Vec::new();
        for tag in &parsed.tags {
            match tag {
                Tag::DefineSprite(sprite) => {
                    let frames = split(&sprite.tags);
                    assert_eq!(frames.len(), usize::from(sprite.num_frames));
                    sprites.insert(sprite.id, frames);
                }
                Tag::PlaceObject(_) | Tag::RemoveObject(_) | Tag::ShowFrame => {
                    main.push(match tag {
                        Tag::ShowFrame => Tag::ShowFrame,
                        other => clone_tag(other),
                    });
                }
                _ => {}
            }
        }
        Self {
            sprites,
            main: split(&main),
            root: BTreeMap::new(),
            ticks: 0,
        }
    }

    /// Applies one frame's tags to a display list; true when the frame stops its timeline.
    fn apply(&self, list: &mut BTreeMap<u16, Object>, tags: &[Tag<'static>]) -> bool {
        let mut stop = false;
        for tag in tags {
            match tag {
                Tag::PlaceObject(p) => match p.action {
                    PlaceObjectAction::Place(character) | PlaceObjectAction::Replace(character) => {
                        if matches!(p.action, PlaceObjectAction::Place(_)) {
                            assert!(!list.contains_key(&p.depth), "Place on depth {}", p.depth);
                        }
                        let matrix = p.matrix.as_ref().map_or(DAffine2::IDENTITY, to_affine);
                        list.insert(p.depth, self.spawn(character, matrix, p.clip_depth));
                    }
                    PlaceObjectAction::Modify => {
                        let object = list
                            .get_mut(&p.depth)
                            .unwrap_or_else(|| panic!("Modify of empty depth {}", p.depth));
                        if let Some(m) = &p.matrix {
                            object.matrix = to_affine(m);
                        }
                    }
                    PlaceObjectAction::None => panic!("never written"),
                },
                Tag::RemoveObject(r) => {
                    list.remove(&r.depth)
                        .unwrap_or_else(|| panic!("Remove of empty depth {}", r.depth));
                }
                Tag::DoAction(bytes) => {
                    assert_eq!(*bytes, STOP, "only stop() is written");
                    stop = true;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        stop
    }

    /// A new object; a sprite shows its first frame at once.
    fn spawn(&self, character: u16, matrix: DAffine2, clip_depth: Option<u16>) -> Object {
        let mut object = Object {
            character,
            matrix,
            clip_depth,
            frame: 0,
            stopped: false,
            children: BTreeMap::new(),
        };
        if let Some(frames) = self.sprites.get(&character) {
            let mut children = BTreeMap::new();
            object.stopped = self.apply(&mut children, &frames[0]);
            object.children = children;
        }
        object
    }

    /// One frame on: the children first (they have their own clocks), then the sprite's own
    /// playhead, which starts over at the end unless a `stop()` held it.
    fn advance(&self, object: &mut Object) {
        for child in object.children.values_mut() {
            self.advance(child);
        }
        let Some(frames) = self.sprites.get(&object.character) else {
            return;
        };
        if object.stopped || frames.len() < 2 {
            return;
        }
        let mut next = object.frame + 1;
        if next >= frames.len() {
            next = 0;
            // Going back to the first frame takes away what later frames put on screen.
            object.children.clear();
        }
        object.frame = next;
        let mut children = std::mem::take(&mut object.children);
        object.stopped = self.apply(&mut children, &frames[next]);
        object.children = children;
    }

    /// Plays the next frame of the movie and says what is on screen.
    pub fn tick(&mut self) -> Flat {
        let mut root = std::mem::take(&mut self.root);
        for object in root.values_mut() {
            self.advance(object);
        }
        self.apply(&mut root, &self.main[self.ticks]);
        self.ticks += 1;
        let mut flat = Flat::default();
        self.flatten(&root, DAffine2::IDENTITY, &[], &mut flat);
        self.root = root;
        flat
    }

    fn flatten(
        &self,
        list: &BTreeMap<u16, Object>,
        parent: DAffine2,
        inherited: &[usize],
        out: &mut Flat,
    ) {
        let mut active: Option<(u16, usize)> = None;
        for (depth, object) in list {
            if active.is_some_and(|(limit, _)| *depth > limit) {
                active = None;
            }
            let transform = parent * object.matrix;
            if let Some(limit) = object.clip_depth {
                // A clip layer is not drawn; what it would draw is the region it clips to.
                let mut region = Flat::default();
                self.draw(object, transform, &[], &mut region);
                out.clips.push(
                    region
                        .shapes
                        .iter()
                        .map(|s| (s.character, s.transform))
                        .collect(),
                );
                active = Some((limit, out.clips.len() - 1));
                continue;
            }
            let mut clips = inherited.to_vec();
            clips.extend(active.map(|(_, index)| index));
            self.draw(object, transform, &clips, out);
        }
    }

    fn draw(&self, object: &Object, transform: DAffine2, clips: &[usize], out: &mut Flat) {
        if self.sprites.contains_key(&object.character) {
            self.flatten(&object.children, transform, clips, out);
        } else {
            out.shapes.push(FlatShape {
                character: object.character,
                transform,
                clips: clips.to_vec(),
            });
        }
    }
}

/// What is on screen after each frame of the movie.
pub fn play(parsed: &Parsed) -> Vec<Flat> {
    let mut player = Player::new(parsed);
    let frames = player.main.len();
    (0..frames).map(|_| player.tick()).collect()
}

/// Re-renders frame `index` of the SWF from what the reader decoded.
pub fn render_swf_frame(parsed: &Parsed, index: usize, background: Rgba8) -> flare_render::Image {
    let mut by_character: HashMap<u16, Vec<Shape>> = HashMap::new();
    for tag in &parsed.tags {
        if let Tag::DefineShape(shape) = tag {
            by_character.insert(shape.id, rebuild(shape));
        }
    }
    let flat = play(parsed).swap_remove(index);
    let mut drawn = Vec::new();
    for placed in &flat.shapes {
        for shape in &by_character[&placed.character] {
            drawn.push(flare_eval::DrawnShape {
                path: Vec::new(),
                shape: std::borrow::Cow::Borrowed(shape),
                phase: 0,
                transform: placed.transform,
                clips: placed.clips.clone(),
                outline: flare_document::OUTLINE_PALETTE[0],
            });
        }
    }
    let clips = flat
        .clips
        .iter()
        .map(|members| {
            members
                .iter()
                .flat_map(|(character, transform)| {
                    by_character[character]
                        .iter()
                        .map(move |shape| flare_eval::ClipShape {
                            shape: std::borrow::Cow::Borrowed(shape),
                            phase: 0,
                            transform: *transform,
                        })
                })
                .collect()
        })
        .collect();
    let frame = flare_eval::Frame {
        placed: Vec::new(),
        shapes: drawn,
        bitmaps: Vec::new(),
        clips,
        warnings: Vec::new(),
    };
    flare_render::render(
        &frame,
        (parsed.stage.0 / 20) as u32,
        (parsed.stage.1 / 20) as u32,
        background,
    )
    .expect("render")
}
