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

The rules are in [AGENTS.md](AGENTS.md) and [docs/engineering.md](docs/engineering.md):

- All input is untrusted and everything that grows has a bound (file sizes, archive entries,
  frame counts, image sizes, preferences, recent lists, history).
- No `unwrap`, `expect` or panics on user data; errors are typed.
- Exports and publishes write beside the target and rename into place, never overwrite, and leave
  nothing behind on failure.
- The MCP server only writes inside an export directory chosen at start-up, accepts plain file
  names only, and caps image sizes.
- Property tests feed random and truncated data to the importers, storage and the editing core;
  see [docs/testing.md](docs/testing.md).
