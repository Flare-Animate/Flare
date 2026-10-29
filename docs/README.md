# Flare docs

Flare is an Adobe Animate replacement. The modern Animate workspace comes first; classic
Flash workspaces and OpenToonz-inspired features follow once the core workflow is solid.

- [architecture.md](architecture.md): crates, the document model, editing, storage, frontends, known gaps
- [engineering.md](engineering.md): toolchain, conventions, verification, reference code policy
- [ui.md](ui.md): the interface, its icons, and how planned features are shown before they exist
- [performance.md](performance.md): how the editor stays responsive, measurements, and the rules new work follows
- [web.md](web.md): the browser build, what it shares with the desktop and what is missing
- [testing.md](testing.md): what is tested, how, and in CI
- [release.md](release.md): release checklist, packaging, the manual smoke test
- [parity.md](parity.md): what the archived C++ project had and what Flare does with it
- [roadmap.md](roadmap.md): milestones, what is done, and the next independent branches
- [decisions/renderer.md](decisions/renderer.md): why vello_cpu is the CPU rasterizer
- [decisions/ui-stack.md](decisions/ui-stack.md): egui, winit, wgpu and the native macOS menu

Docs describe what is true now plus accepted near-term plans. When behaviour changes,
change the doc in the same commit.
