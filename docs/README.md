# Flare docs

Flare is an Adobe Animate replacement. The modern Animate workspace comes first; classic
Flash workspaces and OpenToonz-inspired features follow once the core workflow is solid.

- [ARCHITECTURE.md](ARCHITECTURE.md): crates, the document model, editing, storage, frontends, known gaps
- [ENGINEERING.md](ENGINEERING.md): toolchain, conventions, verification, reference code policy
- [UI.md](UI.md): the interface, its icons, and how planned features are shown before they exist
- [PERFORMANCE.md](PERFORMANCE.md): how the editor stays responsive, measurements, and the rules new work follows
- [BUILDING.md](BUILDING.md): how to build and run on each platform, with the toolchain and submodule
- [WEB.md](WEB.md): the browser build, what it shares with the desktop and what is missing
- [TESTING.md](TESTING.md): what is tested, how, and in CI
- [RELEASE.md](RELEASE.md): release checklist, packaging, the manual smoke test
- [SURFACES.md](SURFACES.md): what the desktop app, the browser build, the command line and the MCP server each do, and the gaps
- [PARITY.md](PARITY.md): what the archived C++ project had and what Flare does with it
- [ROADMAP.md](ROADMAP.md): milestones, what is done, and the next independent branches
- [decisions/RENDERER.md](decisions/RENDERER.md): why vello_cpu is the CPU rasterizer
- [decisions/LOSSLESS.md](decisions/LOSSLESS.md): why an opened file is kept whole and how an unchanged export is the original
- [decisions/UI-STACK.md](decisions/UI-STACK.md): egui, winit, wgpu and the native macOS menu

Docs describe what is true now plus accepted near-term plans. When behaviour changes,
change the doc in the same commit.
