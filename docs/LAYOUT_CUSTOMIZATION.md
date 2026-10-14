# Layout customization

Layouts ("rooms") live in `stuff/profiles/layouts/rooms/<Profile>/`:

| File | Purpose |
|---|---|
| `layouts.txt` | Ordered list of room files (`room1.ini` ...). |
| `roomN.ini` | One room: `name=`, `hierarchy="..."` and `pane_<i>\name=` entries (panel type + geometry). |
| `menubar_template.xml` | Menubar tree; each `<item>` is one command id, i.e. one button/menu entry. |

Profiles shipped: `Animate` (default), `OpenToonz` (classic), `FlipaClip`, `StudioGhibli`.
Default is `CurrentRoomChoice` in `flare/sources/flarelib/preferences.cpp`.

## Customize
1. Copy a profile dir to a new name (e.g. `MyLayout`); it appears in Preferences > Interface > Layout.
2. Edit `roomN.ini` to move/add panes. Every index in `hierarchy` needs a matching `pane_<i>\name=`.
3. Edit `menubar_template.xml` to add, remove or reorder commands.
4. User edits are stored in the user profile dir (`<profile>/layouts/rooms/<Profile>/`) and override the stock files.
5. Export/import = zip or copy the profile dir.

## Agents
An AI agent may edit the files above directly. Validate with `python tests/test_flipaclip_profile.py`
(adapt the profile name) before use.

## Submit a layout
Open a PR adding `stuff/profiles/layouts/rooms/<Name>/`. See CONTRIBUTING.md.
