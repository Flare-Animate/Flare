# Building Flare

Every platform starts the same way. Rust is pinned in `rust-toolchain.toml` (1.99.1, with rustfmt
and clippy); rustup installs it on the first `cargo` command in the checkout. Ruffle is a git
submodule in `third_party/ruffle`, and cargo reads its manifests on every build, so the submodule
is needed everywhere:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare

In a clone made without `--recurse-submodules`, run `git submodule update --init --recursive`.

Then, on any platform:

    cargo run --release -p flare-desktop    # the desktop app
    cargo run -p flare-cli -- --help        # the command line (binary: flare)
    cargo build -p flare-mcp                # the MCP server (binary: flare-mcp)

Before a commit, these must pass (see [ENGINEERING.md](ENGINEERING.md) and [TESTING.md](TESTING.md)):

    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked

## Linux

Install the system libraries the desktop app links against. These are the Debian and Ubuntu package
names used by CI; other distributions name them differently:

    sudo apt-get install pkg-config libasound2-dev libfontconfig1-dev libwayland-dev libxkbcommon-dev \
      libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxcb1-dev

Then the commands above. Test Movie needs Ruffle's desktop player, which is optional for everything
else. Build it from the submodule:

    scripts/build_ruffle.sh

This takes a few minutes and about 1.5 GB. It puts the player at `target/ruffle-desktop/release/ruffle_desktop`.
Set `FLARE_RUFFLE` to that path, or put it on `PATH` as `ruffle`.

## Windows

Rust on Windows uses the MSVC toolchain, which needs the Visual Studio C++ build tools. Install them
from Visual Studio Build Tools (the "Desktop development with C++" workload); the rustup installer offers this too.
Then run the commands above in PowerShell or a terminal.

`scripts/build_ruffle.sh` is a shell script; run it from Git Bash to build the Test Movie player
(`target/ruffle-desktop/release/ruffle_desktop.exe`). Then set `FLARE_RUFFLE` to that file.

## macOS

Install the Xcode command line tools (`xcode-select --install`), which provide the linker. Then the
commands above. Test Movie is built the same way as on Linux (`scripts/build_ruffle.sh`).

### Older Intel Macs (one report, not verified)

Issue #125 reports a build of an older Qt-based version of Flare on an Intel Mac running macOS 11
(Big Sur), using MacPorts for Qt, OpenCV and Boost. The reporter says it built and launched. That
build used the code from before the Rust rewrite, not this workspace, and nothing in this document
was run on it. It is not a supported configuration. The reporter's steps are in the issue.

## Web (browser build)

The browser build is [`apps/flare-web`](../apps/flare-web/README.md), described in [WEB.md](WEB.md).
It needs:

- the WebAssembly target: `rustup target add wasm32-unknown-unknown`
- the `wasm-bindgen` command line tool at the version the lockfile uses: `cargo install wasm-bindgen-cli --version 0.2.129 --locked`
- Node.js with npm

Then:

    apps/flare-web/scripts/build.sh

This writes `apps/flare-web/dist/`. Serve that folder with any static file server.

## Android

The Android app is [`apps/flare-android`](../apps/flare-android/README.md). It needs the Android NDK
(`ANDROID_NDK_HOME`) and the target: `rustup target add aarch64-linux-android`.

    cargo build -p flare-android --target aarch64-linux-android --release

That builds the library only. For an APK for a device or emulator, install
[cargo-apk](https://github.com/rust-mobile/cargo-apk) and run:

    cargo install cargo-apk
    cargo apk build -p flare-android --target aarch64-linux-android

Test Movie is not offered on Android yet. The app's README lists what else differs from the desktop.

## iOS

There is no iOS app. The iOS CI workflow only tries to compile `flare-desktop` for
`aarch64-apple-ios`, and it does not fail when that does not compile. Do not treat it as a build path.
