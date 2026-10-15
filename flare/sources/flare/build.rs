//! Windows: embed the app icon and version info in flare.exe.
//! (Other platforms take the icon from the window at runtime / the package.)
fn main() {
    println!("cargo:rerun-if-changed=assets/icon/flare.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/flare.ico");
        res.set("ProductName", "Flare");
        res.set("FileDescription", "Flare animation editor");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
