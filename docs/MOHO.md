# Moho projects

`flare-moho` reads Moho (Lost Marble, formerly Anime Studio) projects and BVH motion files and
converts them into Flare documents. `flare-import` routes a file to it by name, and `flare import`,
`import_project`, the desktop Open command and the browser Open command all go through that route.

## What is verified and what is spec-only

Verified means a test in the repository runs the behaviour. Real-sample checks were run by hand
against the MIT-licensed `CharWalk.moho` and `CharWalk.mohoproj` from godot_moho_importer
(not committed; see BACKLOG.md "Sources", item 10). They are the only real Moho 13 files used.

| Area | Status | Where it is tested |
|---|---|---|
| ZIP container (`Project.mohoproj` or `Project.animeproj`), raw JSON, UTF-8 BOM | verified (synthetic files, real `CharWalk.moho` imports) | `flare-moho/tests/moho.rs` |
| Moho 13 bone layers: skeleton inside each `BoneLayer` (`layers[i].skeleton.bones`) | verified (real sample: 23 bones become folders) | `flare-moho/tests/moho13.rs` |
| Legacy root `skeleton.bones` (Moho 11-12 as seen in the fixture) | verified (synthetic) | `moho.rs`, `convert.rs` |
| Bone parents: forward references, loops (broken, not followed), out-of-range (made roots) | verified (real sample has forward parents) | `moho13.rs` |
| Bone pose tracks (`anim_pos`, `anim_angle`, `anim_scale`) on the bone's own folder | verified: spans per key, linear and hold | `moho13.rs`, `convert.rs` |
| Vector layers (`MeshLayer`): points, closed and open curves as polylines | verified (real sample: 18 layers) | `convert.rs`, `convert_vector.rs` |
| Vector fill colour: first filled shape's `style.fill_color`; several fills are a warning | verified (synthetic) | `moho13.rs` |
| Bezier handles | **not read**; curves are drawn as polylines | diagnostic on each vector layer |
| Image layers (`ImageLayer`): PNG from the project's own folder, centred at its pixel size, placed through the layer's tracks | verified (synthetic PNG in the test, through `import_path`) | `flare-moho/tests/images.rs`, `flare-import/tests/moho_pictures.rs`, `flare-cli/tests/project/moho.rs`, `flare-mcp/tests/export.rs` |
| Image paths: relative only, no `..`, no drive, no link that leaves the folder, PNG only, size and total limits | verified (failure paths) | `flare-import/tests/moho_pictures.rs` |
| Switch layers (`SwitchLayer`): one symbol per alternative, a span per key | verified (synthetic) | `moho.rs`, `convert.rs` |
| Groups as folders, layer order, visibility | verified (synthetic + real sample) | `convert.rs` |
| Bone-bound layers (`parent_bone` >= 0): they keep their own placement | **gap**: reported as Info on each layer | diagnostic |
| Bone-bound layers following the bone's pose | **gap**, the same as above | - |
| Audio (`audio_path`, `sound_path`): named Unsupported; the key names are unverified | spec-only | `convert.rs` (note) |
| Patch, 3D, text, camera and other layer types | named Unsupported per layer | `convert.rs` |
| Actions (`actions`, `action_refs`) and interpolation other than linear and hold | smooth and ease become linear with a warning; actions are not converted | `convert_anim.rs` |
| Pre-11 `.anme` (plain text, "Anime Studio Project") | refused by name | `moho.rs` |
| Papagayo `.pgo`, `.animepro`, zipped `.anme` | not read (spec-only; no sample) | - |
| Effects, dynamics (bone `bone_dynamics`), IK constraints | named Unsupported on the bone | `convert.rs` |
| Writing a Moho project back | not built | - |

## Reference projects

The format facts (layer type names, `Project.mohoproj` member name, bone and mesh key names,
and the track names) come from godot_moho_importer (MIT, attributed in ROADMAP) and the Moho 13
sample. Other references were read for behaviour only, with no code taken: moho-file-analyzer
(no licence file in the repository), moho-bvh-bones (CC BY-NC-SA 4.0). A web search found no
other open-source Moho parser; see BACKLOG.md "Sources", item 10.

## Limits

Every size and count is bounded (`flare-moho/src/limits.rs`; picture limits in
`flare-import/src/moho_images.rs`). A parent loop is broken, an out-of-range parent becomes a root,
and a bad picture or path is one note on its layer; none of these fail the whole import.
