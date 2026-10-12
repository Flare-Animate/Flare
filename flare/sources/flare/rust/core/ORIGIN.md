# AnimationCraft core in Flare

Copied from `KMSDax/AnimationCraft`, commit
`e3aadad1c9cc6451f39746226123fd1021fd889a`, `crates/ac-core`.
Upstream declares `MIT OR Apache-2.0` in its workspace manifest; this declaration
is retained in this crate's manifest. That revision contains no standalone license
or NOTICE file. Original source comments and attribution are preserved.

Flare's workspace patches ac-core for both Studio and the pinned ac-render, keeping
one shared document type. The standalone manifest replaces upstream workspace
inheritance with identical versions. Flare additions: persisted per-layer row-height
percentage with a backward-compatible 100% default; partial tween clipboard
boundaries retain the original curve over a persisted source-time interval.
Complete tweens retain their original representation. Subsequent pose-editing
parity without materialization remains a tracked Studio P1 requirement.
Keyframes also persist a backward-compatible graphic playback offset and direction:
split/copy/sample operations retain nested animation phase, static and motion span
reversal invert that clock, and movie clips keep their independent global clock.
Graphic first-frame arithmetic uses a widened sum to avoid overflow. Frame-by-frame
conversion samples the original source before boundary splits and retains labels and
scripts only on their authored keys. The frame-clock regression module covers these
changes; it does not claim editable nonlinear boundary parity.
Clipped spans persist TweenBoundary endpoint geometry/hints, source-time start/end
and direction (legacy default zero/forward). Evaluation, guide traversal, duration
retiming, clipboard dependency walks, floating-canvas save normalization, library
usage/deletion and parent-space conversion include that data. Splits preserve both
intervals; clearing compatible split keys rejoins them. Partial bake/clear/remove,
replacement/overwrite paste and reversal preserve exterior curves. Reversal keeps
original curve data and mirrors labels/scripts and nested clocks. Disjoint pastes
do not stretch a preceding retained clip into their gap. Inspector pose editing
still uses a pose-preserving materialization fallback with localized notices.
Studio validates retained time domains. Library bitmap usage/deletion also
includes bitmap fills. Further adaptations must be
listed here and covered by document/history tests. Source formatting has been
normalized with rustfmt. All other engine behavior remains from the recorded revision.
