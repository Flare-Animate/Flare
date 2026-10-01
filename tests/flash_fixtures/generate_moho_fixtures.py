"""Build Moho fixtures covering the cases MohoReader has to survive.

Each fixture is minimal but structurally faithful, and each exercises one
documented behaviour of the format -- so a reader that only handles the happy
path fails here.
"""
import json
import os
import sys
import zipfile

OUT = sys.argv[1] if len(sys.argv) > 1 else "."

# A fixed timestamp for every archive entry.
#
# zipfile.writestr() stamps each entry with the current time, so regenerating
# a fixture produced a byte-different file every time. That matters for
# committed fixtures: `git status` was permanently dirty after any
# regeneration, and a fixture's checksum could not be used to tell whether it
# had really changed. Fixing the timestamp makes generation reproducible, so a
# fixture differs only when its content differs.
_ZIP_EPOCH = (1980, 1, 1, 0, 0, 0)


def zinfo(name):
    """A ZipInfo with a fixed timestamp, so archives are reproducible."""
    zi = zipfile.ZipInfo(name, date_time=_ZIP_EPOCH)
    zi.compress_type = zipfile.ZIP_DEFLATED
    # 0o644: rw-r--r--, so the archive is usable on any host.
    zi.external_attr = (0o644 << 16)
    return zi



def channel(vals, frames=None, ctype="Val"):
    """A Moho animated value: parallel `when`/`val` arrays."""
    return {
        "type": ctype,
        "ref": False,
        "mute": False,
        "when": frames if frames is not None else list(range(len(vals))),
        "val": vals,
        "interp": [{"im": 0, "v1": -1.0, "v2": -1.0, "in": 1, "h": 0,
                    "s": False, "t": 0} for _ in vals],
    }


def doc(layers, version=1038, project_data=None, **extra):
    """Assemble a document. The 1021 generation omits fields the later ones
    carry, so a total reader must not assume their presence."""
    d = {
        "mime_type": "application/x-vnd.lm_mohodoc",
        "version": version,
        "major_version": 1,
        "rev_version": 0,
        "project_data": project_data if project_data is not None else {
            "width": 800, "height": 600, "fps": 24.0,
            "end_frame": 48, "start_frame": 1,
        },
        "styles": [],
        "layers": layers,
    }
    d.update(extra)
    return d


# ---------------------------------------------------------------------------
# 1. Minimal valid rig: one bone layer, one bone, one mesh layer.
# ---------------------------------------------------------------------------
minimal = doc([{
    "type": "BoneLayer",
    "name": "Rig",
    "uuid": "rig-1",
    "visible": True,
    "actions": [{"name": "Head", "pose": 0}],
    "layers": [
        {"type": "MeshLayer", "name": "Body", "uuid": "mesh-1",
         "visible": True, "parent_bone": 0,
         "mesh": {"type": "Mesh", "points": [
             {"type": "Point", "position": channel([0.0, 5.0], ctype="Vec2"),
              "width": channel([1.0]), "parent": -2},
         ], "curves": [], "shapes": []}},
    ],
    "skeleton": {"type": "Skeleton", "binding_mode": 1, "bones": [
        {"type": "Bone", "name": "Root", "parent": -1, "length": 1.0,
         "strength": 0.0,
         "anim_angle": channel([0.0, 0.5, 0.25])},
    ]},
}])

# ---------------------------------------------------------------------------
# 2. A switch layer (lip sync / rig variants) with keyed alternatives.
# ---------------------------------------------------------------------------
switch = doc([{
    "type": "SwitchLayer", "name": "Mouth", "uuid": "sw-1", "visible": True,
    "switch_keys": channel(["MouthA", "MouthB", "MouthA"], ctype="String"),
    "layers": [
        {"type": "MeshLayer", "name": "MouthA", "uuid": "m-a", "visible": True},
        {"type": "MeshLayer", "name": "MouthB", "uuid": "m-b", "visible": True},
    ],
}])

# ---------------------------------------------------------------------------
# 3. The 1021 generation: no doc_uuid, no action_refs, no modified_date, and
#    curve points with no bezier handle weights at all.
# ---------------------------------------------------------------------------
old_gen = doc(
    [{"type": "MeshLayer", "name": "Legacy", "uuid": "old-1", "visible": True,
      "image_path": "images/legacy.png",
      "mesh": {"type": "Mesh", "points": [
          {"type": "Point", "position": channel([0.0], ctype="Vec2")},
      ], "curves": [
          {"type": "Curve", "num_points": 4, "closed": True, "points": [
              # No weight_in/weight_out/offset_in/offset_out: the 1021 shape.
              {"point": 0, "segments_on": True,
               "smoothness": channel([0.0])},
          ]},
      ], "shapes": []}}],
    version=1021)

# ---------------------------------------------------------------------------
# 4. Deeply nested groups with bone binding, and an out-of-range parent
#    index (Moho writes these when a bone was deleted).
# ---------------------------------------------------------------------------
nested = doc([{
    "type": "GroupLayer", "name": "Outer", "uuid": "g-1", "visible": True,
    "layers": [{
        "type": "GroupLayer", "name": "Inner", "uuid": "g-2", "visible": False,
        "layers": [
            {"type": "MeshLayer", "name": "Rigged", "uuid": "m-rig",
             "visible": True, "parent_bone": -1,
             "flexi_bone_subset": "0|1|2",
             "fill_texture_path": "images/skin.png"},
            {"type": "ImageLayer", "name": "Backdrop", "uuid": "i-1",
             "visible": True, "image_path": "images/bg.png"},
            {"type": "TextLayer", "name": "Caption", "uuid": "t-1"},
        ],
    }],
}, {
    "type": "BoneLayer", "name": "Bones", "uuid": "bl-2", "visible": True,
    "skeleton": {"type": "Skeleton", "binding_mode": 1, "bones": [
        {"type": "Bone", "name": "A", "parent": -1, "length": 1.0},
        {"type": "Bone", "name": "B", "parent": 0, "length": 1.0},
        # Deliberately dangling: parent 99 does not exist.
        {"type": "Bone", "name": "Orphan", "parent": 99, "length": 1.0},
    ]},
}])

# ---------------------------------------------------------------------------
# 5. Not a Moho project: valid JSON, wrong mime type.
# ---------------------------------------------------------------------------
wrong_mime = {"mime_type": "application/json", "version": 1038,
              "major_version": 1, "project_data": {}, "layers": []}

# ---------------------------------------------------------------------------
# 6. Malformed: JSON that does not parse.
# ---------------------------------------------------------------------------

fixtures = {
    "minimal_rig": minimal,
    "switch_layer": switch,
    "old_1021_generation": old_gen,
    "nested_and_dangling_bone": nested,
    "not_moho": wrong_mime,
}

os.makedirs(OUT, exist_ok=True)
for name, d in fixtures.items():
    # The ZIP container form, which is what .moho actually is.
    zpath = os.path.join(OUT, name + ".moho")
    with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr(zinfo("Project.mohoproj"), json.dumps(d))
    # The bare document form, which .mohoproj is.
    with open(os.path.join(OUT, name + ".mohoproj"), "w",
              encoding="utf-8") as f:
        json.dump(d, f)
    print(f"  {name}: .moho + .mohoproj")

# Truncated JSON, and a pre-11 plain-text .anme.
with open(os.path.join(OUT, "truncated.mohoproj"), "w", encoding="utf-8") as f:
    f.write('{"mime_type": "application/x-vnd.lm_mohodoc", "version": 1038,')
with open(os.path.join(OUT, "legacy.anme"), "w", encoding="utf-8") as f:
    f.write("Anime Studio Project\n  version 9\n  {\n")
# A ZIP that is not a Moho project at all.
with zipfile.ZipFile(os.path.join(OUT, "plain.zip"), "w") as z:
    z.writestr(zinfo("readme.txt"), "not a moho project")
print("  truncated.mohoproj, legacy.anme, plain.zip")
print("done")
