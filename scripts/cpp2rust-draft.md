# cpp2rust drafts (CI only)

cpp2rust (github.com/Cpp2Rust/cpp2rust) needs LLVM/clang 23, nightly rustc-dev
and ~15 GB of disk, so it cannot be built in the dev container. It runs in
`.github/workflows/cpp2rust-draft.yml` instead. Background: docs/CPP2RUST_TRIAL.md.

## Dispatch

    gh workflow run cpp2rust-draft.yml --ref <branch> -f files="stdfx/hsvutil.cpp stdfx/gammafx.cpp"

`files` = space-separated paths relative to `flare/sources`. Include paths used:
`flare/sources/include` and `flare/sources/<dir of the file>`. The workflow is
manual only (never on push/PR), read-only (`contents: read`), and commits nothing.

## Reading the result

    gh run list --workflow=cpp2rust-draft.yml -L 1
    gh run view <id> --log | sed -n '/=== DRAFT/,/=== END ===/p'
    gh run download <id> -n cpp2rust-drafts      # full output dir

Each file is printed between `=== DRAFT <file> ===` and `=== END ===` (first
60000 bytes, plus exit code and stderr).

## How drafts are used

A draft is a reference, not a deliverable. Output is expected to be verbose
(`Ptr<T>`/refcount runtime, libcc2rs) and may fail on Qt/tcommon headers; prefer
Qt-free leaf files. Per docs/RUST_REWRITE.md:
1. Hand-fix or hand-port into the Rust module that replaces the original
   (same location, registered in the crate root / workspace).
2. Remove the libcc2rs dependency; use plain slices/structs.
3. Differential-test against the original C++ on random inputs before the
   C++ is retired. No draft lands without that test.
