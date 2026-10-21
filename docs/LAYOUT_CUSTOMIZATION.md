# Layout customization

Layouts ("rooms") live in `stuff/profiles/layouts/rooms/<Profile>/`:

| File | Purpose |
|---|---|
| `layouts.txt` | Ordered list of room files (`room1.ini` ...). |
| `roomN.ini` | One room: `name=`, `hierarchy="..."` and `pane_<i>\name=` entries (panel type + geometry). |
| `menubar_template.xml` | Menubar used by every room that has no `roomN_menubar.xml`: `<menubar>` of `<menu title="...">` (nestable) holding `<command>MI_...</command>` and `<separator/>`. Unknown command ids inside a menu are skipped silently. |
| `roomN_menubar.xml` | Optional menubar for just that room (StudioGhibli uses these). |

Profiles shipped: `Animate` (default), `AnimateLegacy` (previous default), `OpenToonz` (classic),
`FlipaClip`, `StudioGhibli`. What each is for, the Animate design, history and how to restore:
[LAYOUT_PROFILES.md](LAYOUT_PROFILES.md). Default is `CurrentRoomChoice` in
`flare/sources/flarelib/preferences.cpp` (the chosen profile is kept; a missing one falls back to `Animate`).

Every profile must expose all UI options (every registered command in every room's menubar, hidden
ones under **All Commands**). Check with `python scripts/check_layout_profiles.py`.

## Customize
1. Copy a profile dir to a new name (e.g. `MyLayout`); it appears in Preferences > Interface > Layout.
2. Edit `roomN.ini` to move/add panes. Every index in `hierarchy` needs a matching `pane_<i>\name=`.
3. Edit `menubar_template.xml` to add, remove or reorder commands, then run
   `python scripts/check_layout_profiles.py --sync-all-commands` so anything you took out reappears under
   **All Commands** (the check fails otherwise).
4. User edits are stored per profile and user in `<profile folder>/layouts/personal/<Profile>.<user>/` and
   override the stock files (so edited stock files only reach users after Windows > Workspace > Reset Room Layout).
5. Export/import = zip or copy the profile dir, or `python tools/layout_tool.py export|import|validate`.
6. Command bar: presets for each profile are in `stuff/profiles/layouts/settings/presets/`; load one with
   *Import Layout...* in the Command Bar editor. Shortcut presets (`shortcuts/*.ini`, incl. `defopentoonz.ini`) are
   chosen in the Shortcuts popup; each defines every shortcut key.

## Agents
An AI agent may edit the files above directly. Validate with `python scripts/check_layout_profiles.py` and
`python tests/test_layout_profiles.py` before use. Hierarchy rules: the string is `<maximised pane or -1> <1 = root stacks top-to-bottom> [ ... ]`,
orientation alternates per nesting level, and pane geometry must describe the same arrangement.

## Submit a layout
Open a PR adding `stuff/profiles/layouts/rooms/<Name>/`. See CONTRIBUTING.md.
