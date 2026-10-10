# Layout customization

Flare ships layout profiles in `stuff/profiles/layouts/rooms/<Profile>/`:
`Default` (Animate, the default `CurrentRoomChoice`), `OpenToonz` (classic OpenToonz rooms), `FlipaClip` (novice), `StudioGhibli`.
Pick one in Preferences. Edits made in the app are saved per user in `<profile>/layouts/personal/<Profile>.<user>/` and override the shipped copy.

## File schema

| File | Content |
|---|---|
| `layouts.txt` | whitespace-separated room files, tab order |
| `roomN.ini` | Qt settings: `name=<room name>`, `pane_K\name=<panel>` plus geometry, `hierarchy="..."` |
| `menubar_template.xml` | `<menubar><menu title=".."><command>MI_*</command><separator/><submenu title="..">..</submenu></menu></menubar>` |
| `roomN_menubar.xml` | optional per-room menubar, same schema |
| `../toolbars/maintoolbar.xml` | `<toolbar size="30">` one tool id per line (`T_Pen`), `-` is a separator |
| `../settings/commandbar.xml`, `xsheettoolbar.xml` | one command id (`MI_*`) per button |

Rule: every `pane_K` index in a room must appear exactly in its `hierarchy` list (`-1` = split marker). Per-button editing = add, remove or reorder `<command>` / tool-id lines.

## Edit with a script or an AI agent

```
python tools/layout_tool.py export stuff/profiles/layouts/rooms/Default -o my.json   # one JSON bundle: {"name": "...", "files": {"room1.ini": "...", ...}}
# edit my.json (rename "name", change file contents)
python tools/layout_tool.py import my.json -o stuff/profiles/layouts/rooms           # writes dir, then validates
python tools/layout_tool.py validate stuff/profiles/layouts/rooms/MyLayout
```

Agent instructions: edit only the `files` strings, keep command ids from existing menubars, and require `validate` to print `ok`.

## Share a layout

Add `rooms/<Name>/`, run `python tests/test_layout_tool.py`, and open a PR (see CONTRIBUTING.md).
