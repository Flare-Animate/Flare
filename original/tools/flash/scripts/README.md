# Flare JSFL Script Library

Reference JSFL scripts for use with Flare's Flash/Adobe Animate import
pipeline. These scripts can be imported via **File → Import Flash / Animate**
or loaded directly in Adobe Animate as a command script.

## Scripts

### `increment_instances.jsfl`

**Author request:** [Custom toolbar #11](https://github.com/Flare-Animate/Flare/issues/11)

Shifts the frame number of all instances of the selected symbol on **every
timeline** in the document. Prompts for:

- **Start frame** — only instances at or after this frame are shifted.
- **Shift amount** — number of frames to add (positive) or subtract (negative).

### `add_instance_to_symbol.jsfl`

**Author request:** [Custom toolbar #11](https://github.com/Flare-Animate/Flare/issues/11)

Adds the currently selected group on the stage as a new keyframe in a library
symbol, preserving the group's pivot-to-origin alignment. Prompts for a
symbol name:

- **Empty** — adds to the currently selected library item.
- **Existing symbol name** — appends a frame to that symbol.
- **New symbol name** — creates a new graphic symbol first.

### `bake_transform.jsfl`

**Author request:** [Custom toolbar #11](https://github.com/Flare-Animate/Flare/issues/11)

Bakes the transform (scale, rotation, skew, position) of the selected symbol
instance into the symbol's artwork, then applies the **inverse** transform to
every instance of that symbol across all timelines so the appearance is
unchanged. The frame scope can be:

- **Loop** — applies to all frames in the symbol.
- **Play Once** — applies from the instance's `firstFrame` onwards.
- **Single Frame** — applies to one frame only.

## Usage in Flare

1. Go to **File → Import Flash / Animate**.
2. Select a `.jsfl` file.
3. The import dialog will show the detected function names and JSFL API calls.
4. The script is copied to the project's reference folder for documentation.

> **Note:** Full JSFL execution (calling `fl.*` APIs at runtime) requires
> Adobe Animate. These scripts are provided as reference implementations that
> Flare documents and stores; future JSFL runtime support is tracked in
> [#52](https://github.com/Flare-Animate/Flare/issues/52).
