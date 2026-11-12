//! On macOS, gives the program an Info.plist of its own, inside the binary. A packaged app reads
//! the one in its bundle; a program run as it was built has none, and macOS then names it in
//! the menu bar after its file, "flare-desktop". With this section the system finds the name
//! "Flare" there. It holds no bundle identifier, so a build run from a terminal does not share
//! the installed app's permissions or settings.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let mut plist = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" ",
        "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
        "<plist version=\"1.0\">\n<dict>\n",
    ));
    for (key, value) in [
        ("CFBundleName", "Flare"),
        ("CFBundleDisplayName", "Flare"),
        ("CFBundleShortVersionString", version.as_str()),
    ] {
        // Writing to a String cannot fail.
        let _ = writeln!(plist, "    <key>{key}</key>\n    <string>{value}</string>");
    }
    plist.push_str("    <key>NSHighResolutionCapable</key>\n    <true/>\n</dict>\n</plist>\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("Info.plist");
    if let Err(error) = std::fs::write(&out, plist) {
        println!("cargo:warning=could not write the embedded Info.plist: {error}");
        return;
    }
    println!(
        "cargo:rustc-link-arg-bin=flare-desktop=-Wl,-sectcreate,__TEXT,__info_plist,{}",
        out.display()
    );
}
