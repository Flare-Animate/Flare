# Game description and game runs

Flare can describe a game in a project: entities with components, the world's gravity, and the
library symbols and sounds they use. A project can be run as a game headlessly, stepping frames
under a scripted input, and exported as a game description with a sprite sheet. The crate is
`flare-game` (foundation layer): a pure world with no window, sound card, scripting engine or file
access. The archived C++ `tgame` scaffold (`original/flare/sources/common/tgame`) was the starting
point; this page says what was ported, what it does now, and what is still open.

## Status

Built: the world, the seven systems, scripted runs, the description's commands and storage.
Not yet: the game export, the `flare game` commands, the MCP tools and the desktop and browser
panel (each is a gap recorded in [SURFACES.md](SURFACES.md)).

## The model

- **Entity**: an ID (a positive integer, handed out in order and never reused in a game), a name
  (its tag, 1 to 128 characters, no control characters), an optional library symbol it draws, and
  at most one component of each kind.
- **Component**: data only. Nine kinds, each with named fields and defaults (`flare game
  set-component` and MCP send them as `name=value`; an unknown field is an error):

  | Kind | Holds | Needs |
  |---|---|---|
  | `transform` | position, rotation, scale, pivot | nothing |
  | `sprite` | frames, frame rate, loop, visibility, alpha, draw order | transform |
  | `body` | kind (`static`, `kinematic`, `dynamic`), size, velocity, category and mask bits | transform |
  | `control` | the four keys and speed; moves a body | body |
  | `camera` | viewport, zoom, the entity it follows, lerp, bounds | transform |
  | `ui` | kind, size, text, whether it reacts to the pointer | transform |
  | `audio` | a sound of the document, loop, play on start, volume | transform |
  | `particle` | emission rate, life, auto play | transform |
  | `script` | a name | transform |

- **Requirements** are checked on every edit: a component whose requirement is missing is refused,
  and so is removing a component another one needs, or an entity a camera follows.
- **Limits**, all in `flare-game`'s `limits.rs`: 4096 entities, 1024 bodies, coordinates within
  one million pixels, 240 frames a second at most, 100 000 frames in one run, 4096 events in one
  step, and 100 000 events in one run. A value over a limit is refused, never clipped.
- **Gravity** is one vector for the world, in pixels per second squared.

## The world and the step

`World::from_spec(spec, frames_per_second)` builds the running world; the step is `1 / fps`
seconds. `World::step(&Input)` runs the seven systems once each, in this order, and returns the
events of that step. The step runs on a copy and is kept only when every system finishes, so a
refused step changes nothing.

`play(spec, fps, frames, script)` runs a whole game: frame `i` takes `script[i]`, and frames past
the script take no input. It returns the events with their frame numbers and a digest of the final
state. The digest is FNV-1a over every entity's numbers, so two runs of the same description and
script agree exactly, and a different script almost always differs.

Input is text, one frame per `;`-separated part, keys as `,`-separated names, and a press as
`click:x:y`:

    right;right,down;;click:40:12

Keys are `left`, `right`, `up`, `down`, `space` and `enter`. A NaN or infinite pointer is refused.

## The seven systems

The C++ scaffold registers eight system classes; its docs count seven, because the transform
system has no work (the C++ comment says "parent transform propagation would go here", and there is
no hierarchy). The port runs these seven:

| System | What the port does | What the C++ had |
|---|---|---|
| Script | Nothing. Scripts are preserved as data and never run (PARITY) | `ScriptSystem::update` ran nothing |
| Physics | Controls set velocities; dynamic bodies fall under gravity; bodies move; overlapping bodies whose masks meet are reported as a collision when they begin and when they part. Nothing is pushed apart | Box2D was never linked; the step is commented out |
| Camera | Follows its target by `lerp * dt` (capped at 1) and clamps its centre to bounds | The lerp and the clamp were comments |
| Sprite | Advances the frame at the sprite's frame rate, looping or holding its last frame; `sprites_in_order` gives the visible sprites lowest order first | Advancing was a comment; the sort by order was real |
| Particle | Counts particles at the emitter's rate and reports `Emit` events; the particles are not simulated | `update` and `createEmitter` were empty |
| Audio | Reports a `Sound` event on the first step for each audio component that plays on start | `playSound` was empty |
| UI | A press inside an interactive element reports a `Click` | The hit test was a comment |

The camera's view maps a world point to the screen as `(p - centre) * zoom + viewport / 2`. The
C++ transform translated and scaled without subtracting the camera's own position, so the port
differs there on purpose.

## Where the port stops

- No physics engine: no contact response, no rotation, circles or polygons (the body is a box).
- No scripts: ActionScript and the C++ script engines are not run.
- No sound output, particle simulation or input device: a surface plays the `Sound` event and draws
  the particles it is told about; the input comes from a script or a surface.
- No asset loading in the world: a symbol or sound is named by ID; the game export and the
  surfaces resolve it.

## In the project

A `Document` has an optional game (`Document::game`, `None` for a project with no game). It is
changed only through command batches in `flare-commands`:

| Command | Effect |
|---|---|
| `AddGameEntity(EntitySpec)` | Adds an entity; its ID is the caller's, from `GameSpec::next_id` |
| `RemoveGameEntity(id)` | Removes it; refused while a camera follows it |
| `SetGameComponent { entity, component }` | Sets one component, replacing one of its kind |
| `RemoveGameComponent { entity, kind }` | Removes one; refused while another component needs it |
| `SetGameGravity { x, y }` | Sets the world's gravity |

A batch is validated once, as a whole, like every other batch: a refused command fails the batch and
changes nothing, and the batch is one undo step. The document checks what the description names:
an entity's symbol must be in the library, and an audio component's sound must be in the sound
library. So removing a symbol or a sound that the game uses is refused by the same rule.

A project with a game is stored with a `game` record and schema version 3, which a build without
games refuses by version. A project without one keeps the version it had, so it opens in older
builds. The record is the description as CBOR; reading it goes through the same checks as an edit,
so a damaged file is refused rather than run.
