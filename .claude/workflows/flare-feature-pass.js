export const meta = {
  name: 'flare-feature-pass',
  description: 'Per feature: implement (opus, worktree) -> test (haiku) -> fix once. Reusable by session + Flare cloud routine.',
  whenToUse: 'args = [{id, goal, refs?, hard?}]. hard=true -> opus, else sonnet. Tests always haiku.',
  phases: [{ title: 'Build' }, { title: 'Test' }, { title: 'Fix' }],
}
const RES = {
  type: 'object',
  properties: { ok: { type: 'boolean' }, branch: { type: 'string' }, notes: { type: 'string' } },
  required: ['ok', 'notes'],
}
const rules = 'FIRST invoke Skill anthropic-skills:caveman and anthropic-skills:ponytail and follow them (terse caveman output, laziest working solution, incl. in thinking). Token budget is tight: no exploration sprawl, no re-reading, no long logs, batch tool calls, grep before read, final report <=6 lines. Haiku for tests/mechanical work only. Repo: Flare (OpenToonz fork, C++/Qt, CMake). Be terse. Minimal diff, reuse existing code. Commit on your worktree branch. Do NOT push or merge. Never mark done without a runnable check.'
const items = Array.isArray(args) ? args : []
if (!items.length) return 'no args'
const out = await pipeline(
  items,
  f => agent(`${rules}\nFeature: ${f.goal}\nReference repos (port ideas/code, keep licenses): ${(f.refs || []).join(', ') || 'none'}\nReturn ok, branch name, notes.`,
    { label: `build:${f.id}`, phase: 'Build', schema: RES, isolation: 'worktree', model: f.hard ? 'opus' : 'sonnet' }),
  (b, f) => !b ? null : agent(`${rules}\nIn branch ${b.branch}: run/verify feature "${f.goal}". Run existing tests + smoke checks, report failures precisely. ok=true only if all pass.`,
    { label: `test:${f.id}`, phase: 'Test', schema: RES, model: 'haiku' }).then(t => ({ b, t })),
  (r, f) => !r ? null : r.t && r.t.ok ? r : agent(`${rules}\nFix failures in branch ${r.b.branch} for "${f.goal}":\n${r.t ? r.t.notes : 'tester died'}`,
    { label: `fix:${f.id}`, phase: 'Fix', schema: RES, isolation: 'worktree', model: f.hard ? 'opus' : 'sonnet' }).then(x => ({ b: x || r.b, t: x })),
)
return out.map((r, i) => ({ id: items[i].id, branch: r && r.b && r.b.branch, ok: !!(r && r.t && r.t.ok), notes: r && r.t && r.t.notes }))
