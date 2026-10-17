//! Diagnose a .fla: how long unzipping and reading take, what it contains, the warnings.
//! cargo run --release -p ac-xfl --example fla_probe -- file.fla
fn main() {
    let path = std::env::args().nth(1).expect("usage: fla_probe <file.fla>");
    let t = std::time::Instant::now();
    let bytes = std::fs::read(&path).unwrap();
    let pkg = match ac_xfl::unzip(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("unzip failed after {:?}: {e}", t.elapsed());
            std::process::exit(1);
        }
    };
    eprintln!("unzipped {} files in {:?}", pkg.len(), t.elapsed());
    let t = std::time::Instant::now();
    match ac_xfl::read_package(&pkg) {
        Ok(im) => {
            let d = &im.doc;
            eprintln!("read in {:?}: {}x{} @{} fps, {} scene layers, {} frames, {} symbols, {} bitmaps, {} sounds", t.elapsed(), d.width, d.height, d.fps, d.scene.layers.len(), d.scene.frame_count(), d.library.len(), d.bitmaps.len(), d.sounds.len());
            for w in im.warnings.iter().take(60) {
                eprintln!("  warn: {w}");
            }
        }
        Err(e) => eprintln!("read failed after {:?}: {e}", t.elapsed()),
    }
}
