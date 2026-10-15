#!/bin/sh
# Cross-build the Windows exes + installer from Linux (MinGW + NSIS).
#   apt install gcc-mingw-w64-x86-64 nsis
#   rustup target add x86_64-pc-windows-gnu      (or, offline: put the std source
#   in rustlib/src/rust/library and this script builds std itself)
# Output: target/x86_64-pc-windows-gnu/release/{flare,flare-cli}.exe
#         and Flare-<version>-setup.exe next to them.
set -e
cd "$(dirname "$0")/.."
T=x86_64-pc-windows-gnu
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
if rustup target list --installed 2>/dev/null | grep -q "^$T$"; then
  cargo build --release -p ac-app -p ac-cli --target $T
else
  RUSTC_BOOTSTRAP=1 cargo build --release -p ac-app -p ac-cli --target $T -Zbuild-std=std,panic_abort
fi
V=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
makensis -V2 -DVERSION="$V" -DBINDIR="../../target/$T/release" packaging/windows/installer.nsi
ls -la target/$T/release/*.exe
