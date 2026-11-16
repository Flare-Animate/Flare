export const meta = {
  name: 'flare-feature-pass',
  description: 'Per feature: implement (worktree) -> test -> fix once. Reusable by session and routine.',
  whenToUse: 'args = [{id, goal, refs?, hard?}]. tier: haiku(simple)|sonnet(mid)|opus(hard); hard=true -> opus. Tests always haiku.',
  phases: [{ title: 'Build' }, { title: 'Test' }, { title: 'Fix' }],
}
const RES = {
  type: 'object',
  properties: { ok: { type: 'boolean' }, branch: { type: 'string' }, notes: { type: 'string' } },
  required: ['ok', 'notes'],
}
// Terse by design: the budget is real, the checks are real, and a runnable one at the end
// is the difference between done and claimed. The working rules are AGENTS.md; the layers
// and the document model are docs/ARCHITECTURE.md.
const rules = `FIRST read AGENTS.md and follow it. Token budget is tight: no exploration sprawl, no re-reading, no long logs; batch tool calls, grep before read; final report <= 6 lines. Haiku for tests and mechanical work only. All code is Rust: crates in crates/ under their layer (foundation, editing, io, adapters, services, presentation), binaries in apps/; behaviour lives in the libraries, an app is only a window or a protocol. Safe Rust, typed errors in libraries, no unwrap/expect/panic on user data, every Rust file under 500 lines, tests in their own files. Every behaviour gets a test that can fail, including the failure paths; do not weaken a test to pass it. Documents change only through validated command batches. Docs describing behaviour change in the same commit. Small conventional commits (feat(scope), fix, refactor, docs, chore, test). Commit on your worktree branch. Do NOT push or merge. Never mark done without a runnable check.`
const items = Array.isArray(args) ? args : []
if (!items.length) return 'no args'
const out = await pipeline(
  items,
  f => agent(`${rules}\nFeature: ${f.goal}\nReference repos (port ideas, respect licences, clean-room unless the licence permits copying): ${(f.refs || []).join(', ') || 'none'}\nReturn ok, branch name, notes.`,
    { label: `build:${f.id}`, phase: 'Build', schema: RES, isolation: 'worktree', model: f.tier || (f.hard ? 'opus' : 'sonnet') }),
  (b, f) => !b ? null : agent(`${rules}\nIn branch ${b.branch}: run/verify feature "${f.goal}". Run the gate: cargo fmt --all --check, cargo clippy --workspace --all-targets --locked -- -D warnings, cargo test --workspace --locked. Report failures precisely (file, line, message). ok=true only if all pass and the feature's own tests exercise it.`,
    { label: `test:${f.id}`, phase: 'Test', schema: RES, model: 'haiku' }).then(t => ({ b, t })),
  (r, f) => !r ? null : r.t && r.t.ok ? r : agent(`${rules}\nFix failures in branch ${r.b.branch} for "${f.goal}":\n${r.t ? r.t.notes : 'tester died'}. Fix the behaviour the test pins, not the test; smallest change first.`,
    { label: `fix:${f.id}`, phase: 'Fix', schema: RES, isolation: 'worktree', model: f.tier || (f.hard ? 'opus' : 'sonnet') }).then(x => ({ b: x || r.b, t: x })),
)
return out.map((r, i) => ({ id: items[i].id, branch: r && r.b && r.b.branch, ok: !!(r && r.t && r.t.ok), notes: r && r.t && r.t.notes }))
