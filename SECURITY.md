# Security policy

Flare opens files from other people (`.flare`, `.fla`/XFL, SWF, images, sounds, fonts) and can be
driven by an AI agent over MCP, so we treat every input as untrusted.

## Supported versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | Yes       |
| older (the C++ code in `original/`) | No |

Only the latest release and `main` receive fixes.

## Reporting a vulnerability

**Do not open a public issue.** Report privately through GitHub:
<https://github.com/Flare-Animate/Flare/security/advisories/new>

If you cannot use GitHub, ask a developer for a private channel on
[Discord](https://discord.com/invite/JpeScW8Awa) without describing the problem in public.

Please include the file or input that triggers it, the version or commit, your platform, and what
you saw (crash, hang, memory growth, file written outside where it should be, and so on). A small
reproducer is the most useful thing you can send.

You can expect an acknowledgement within a week. We will tell you when a fix is released and
credit you unless you prefer not to be named.

## What counts

- A crash, hang or unbounded memory/disk use caused by a crafted file or MCP/CLI input.
- Writing, overwriting or reading a file the user did not choose (path traversal in archives,
  exports, MCP tool arguments).
- Anything that runs code from a document. Flare does not run ActionScript; it is kept as data.
- Memory-unsafety. The workspace is safe Rust; `unsafe` is not used in our crates.

Not security issues: a file Flare does not import faithfully (open a normal issue) or a bug that needs
the attacker to already control your machine.

## How the code defends itself

The rules are in [AGENTS.md](AGENTS.md) and [docs/ENGINEERING.md](docs/ENGINEERING.md):

- All input is untrusted and everything that grows has a bound (file sizes, archive entries,
  frame counts, image sizes, preferences, recent lists, history).
- No `unwrap`, `expect` or panics on user data; errors are typed.
- Exports and publishes write beside the target and rename into place, never overwrite, and leave
  nothing behind on failure.
- Archives are read through their central directory, the index other tools inspect, and entries
  are never extracted to a path taken from the archive; an entry whose local and central names
  disagree is read under its central name. The pinned `zip` also rejects a symlink target longer
  than 4096 bytes before reserving the size it declares, on both of its extracting paths, and
  Flare calls neither of them.
- The MCP server only writes inside an export directory chosen at start-up, accepts plain file
  names only, and caps image sizes.
- Property tests feed random and truncated data to the importers, storage and the editing core;
  see [docs/TESTING.md](docs/TESTING.md).

## Dependencies

CI runs `cargo audit` on `Cargo.lock` for every push. It fails on advisories about vulnerabilities
and passes on the "unmaintained" and "unsound" notes. Two vulnerability advisories are ignored in
[`.cargo/audit.toml`](.cargo/audit.toml): `RUSTSEC-2026-0194` and `RUSTSEC-2026-0195`, quadratic
and unbounded work while parsing XML, which are fixed in `quick-xml` 0.41. Flare's own crates use
`quick-xml` 0.42; the affected 0.37 comes only with the pinned Ruffle submodule, and the player
meets it while running a movie rather than before, so the movie is the one the person chose to
play. Removing the ignore means moving the Ruffle pin to a version that has fixed it.

The notes it passes are tracked: `ttf-parser` (through the window-decoration font code),
`proc-macro-error` and `paste`, `chrono` (through Ruffle), the `zbus` family (through the file
dialogs; `zbus_names`, `zvariant` and `zvariant_derive` are unmaintained as separate crates now
that `zbus` 6 has absorbed them), and the unsoundness in `glib`'s `VariantStrIter` (through the
menu library's GTK support, which only exists in its Linux build; Flare uses that library on
macOS only, so `glib` is in `Cargo.lock` but not in any shipped program). They are tracked and
will be replaced when their parents move.
