# UI assets

The following open assets were sourced from the local PhotoCraft checkout
(`storytold/photocraft`, revision `56b192453e04994666d6532efb43a03d4f1d494f`).

- `fonts/JetBrainsMono-Regular.ttf`: JetBrains Mono, SIL Open Font License 1.1.
  Full notice: `fonts/OFL-JetBrainsMono.txt`.
- `icons/trash.svg`: original Flare vector, under the repository BSD-3-Clause license.
- Other `icons/*.svg`: selected Lucide icons, ISC license, including Feather-derived
  icons under MIT. Full notices: `icons/LICENSE-lucide.txt`. The renderer normalizes
  the stroke color and weight in memory, then applies the current theme tint.

Segoe UI is loaded from the Windows installation at runtime. No Microsoft font
files, PhotoCraft/ArtCraft brand artwork, or Adobe resources are bundled here.
The native UI implements its own components using PhotoCraft's public design
patterns; it does not include PhotoCraft's branding or proprietary assets.

Filled cursor/transport shapes in `theme.rs` are original Flare geometric painters
under the repository BSD-3-Clause license. They do not use extracted Adobe assets.
