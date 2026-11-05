# flare-android

Flare as an Android app. It is not a second implementation: the shell the desktop shows
(`flare-desktop`'s `ui`) runs here on the system's own `NativeActivity`, drawing the same
workspace. This crate is only what a phone gives in place of a desk: the activity, and the
app's own data directory standing in for the per-user paths (see `flare-platform`'s
`set_app_root`).

## Building

The CI (`.github/workflows/workflow_android.yml`) builds only the library, which proves the
whole tree compiles for Android:

```sh
cargo build -p flare-android --target aarch64-linux-android --release
```

It needs an NDK (`ANDROID_NDK_HOME`) and the compiler and linker the target names, which the
workflow sets as `CC_aarch64_linux_android` and `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER`.

An APK for a device or an emulator comes from
[cargo-apk](https://github.com/rust-mobile/cargo-apk), which reads this crate's
`AndroidManifest.xml`:

```sh
cargo install cargo-apk
cargo apk build -p flare-android --target aarch64-linux-android
cargo apk run -p flare-android --target aarch64-linux-android
```

## What is known to differ from the desktop

- Test Movie is off (the desktop feature that runs Ruffle inside the process); a phone build
  needs `flare-desktop`'s `ruffle` feature and a GPU check on a device before it is offered.
- No opening of project files from the phone's storage yet: the Storage Access Framework
  picker has to be wired into the workspace's file dialogs. The app starts on a new document.
- A pen's pressure through the touch screen is not wired to the Brush yet.
- Sound playback is not exercised on a device yet.

These are recorded in [docs/SURFACES.md](../../docs/SURFACES.md).
