# cpp2rust drafts -> draft PR

cpp2rust (github.com/Cpp2Rust/cpp2rust) needs LLVM/clang 23, nightly rustc-dev
and ~15 GB of disk, so it cannot be built in the dev container. It runs in
`.github/workflows/cpp2rust-draft.yml`. Background: docs/CPP2RUST_TRIAL.md.

That workflow used to be manual-only and commit nothing -- it printed drafts into
the job log. It now also **opens a draft pull request** with the auto-converted
Rust placed where the port policy requires it: beside the C++ it came from, same
base name, `foo.cpp` -> `foo.rs`.

## What it will not do

* never merge -- every PR is opened with `--draft`;
* never modify or delete the original `.cpp`;
* never touch `flare_formats.rs`, so the drafts are **not registered** and
  therefore compile nowhere. A draft cannot break the build until a human has
  read it. (`flare_formats::tests::module_registry_matches_the_tree` enforces
  the other half of this: a registered module must not still have its `.cpp`.)
* never draft a file that already has a `.rs` sibling.

Every generated file starts with an `AI-GENERATED DRAFT` banner naming the cpp2rust
commit, and ends the file by asking for that banner to be deleted as the last step.

## Triggers

* **nightly**, 04:17 UTC (off the hour: scheduled workflows queue behind the
  busiest minutes);
* `workflow_dispatch`, with an optional explicit file list;
* `repository_dispatch` with `event_type: cpp2rust-draft`.

Permissions are `contents: write` + `pull-requests: write` (it has to push a
branch and open a PR). The checkout uses `persist-credentials: false` and
`gh auth setup-git` supplies the credential only for the push, so no token is
left in `.git/config`.

## Dispatch

    gh workflow run cpp2rust-draft.yml --ref <branch> -f files="stdfx/hsvutil.cpp stdfx/gammafx.cpp"
    gh workflow run cpp2rust-draft.yml --ref <branch> -f batch_size=3     # take the next 3 candidates
    gh workflow run cpp2rust-draft.yml --ref Conversion-ft-animationCraft -f base=Conversion-ft-animationCraft
    gh api -X POST repos/:owner/:repo/dispatches -f event_type=cpp2rust-draft

Paths are relative to `flare/sources`. Include paths used: `flare/sources/include`
and `flare/sources/<dir of the file>`.

**`base` matters.** The drafts branch off whatever `base` resolves to, and the candidate
list comes from that branch's tree — so a nightly run off `master` will re-propose files
that `Conversion-ft-animationCraft` has already ported. Dispatch with `-f base=...` when
you want drafts for the rewrite branch.

With no `files`, the batch comes from

    python scripts/port_batch.py --paths N

which prints machine-readable paths, one per line. The workflow deliberately does
**not** parse `--batch`'s human-facing table -- that format is free to change, and
a CI job depending on column widths is a CI job that breaks when someone tidies a
docstring.

The branch is `cpp2rust/draft-<run number>`, so reruns are idempotent and each run
is traceable.

## Reading the result

    gh pr list --label "" --search "cpp2rust draft"
    gh run list --workflow=cpp2rust-draft.yml -L 5
    gh run view <id> --log | sed -n '/=== DRAFT/,/=== END ===/p'
    gh run download <id> -n cpp2rust-drafts-<run>   # raw output + logs

The PR body carries a table of source file -> drafted path -> cpp2rust exit code,
so a run that produced nothing for a file is visible without opening the log.

## How drafts are used

A draft is a reference, not a deliverable. Output is expected to be verbose
(`Ptr<T>`/refcount runtime, libcc2rs) and may fail on Qt/tcommon headers; prefer
Qt-free leaf files. Per docs/RUST_REWRITE.md:

1. Read the draft and the original `.cpp` side by side and fix every mistranslation.
2. Remove the libcc2rs dependency; use plain slices/structs.
3. **Differential-test against the original C++** before the C++ is retired. A
   golden captured from the original, rebuilt from git history, is the only real
   proof -- see the generators in `tests/native/`. No draft lands without that.
4. Give the module a C ABI (`extern "C"` + the panic guard; never unwind across FFI).
5. Rewrite the header as a thin forwarder, delete the `.cpp`, register the module
   in `flare_formats.rs`.
6. Delete the AI banner.