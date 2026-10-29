# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - not yet released

The first release of the Rust rewrite.

### Added
- A document model with scenes, layers (including mask, folder and motion guide layers), frame
  spans, shapes with strokes and gradients, symbols, and sounds. Every change is a validated
  command batch and one undo step.
- Drawing tools: selection, subselection, free transform, lasso, pen, text, line, rectangle,
  ellipse, polystar, pencil, brush, paint bucket, ink bottle, eyedropper, eraser, hand, zoom.
- Timeline with frames, keyframes, layers, folders, onion skin, linear motion tweens and motion
  guides, and playback with sound.
- Panels that dock and move: tools, properties, color, align, transform, library, history, output.
- Several documents in a window as tabs, Open Recent, Save All, autosave and crash recovery,
  Preferences.
- Files: the `.flare` project container, XFL/FLA import and export, PNG and GIF export, and SWF
  publish. Flash fixtures are tested against.
- The `flare-cli` command line (inspect, render, convert) and an MCP server for agents.
- Layer outline colors (timeline chip, `.flare` and XFL) and View > Outline Mode.
- Window, Help (About, Keyboard Shortcuts) menus that do something; Insert > New Symbol; Edit > Paste in Place (Paste now centers on the stage).
- An experimental web build.

[Unreleased]: https://github.com/Flare-Animate/Flare/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Flare-Animate/Flare/releases/tag/v0.1.0
