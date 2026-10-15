//! The runtime's per-pixel filter stages (glow, shadow variants, bevel, adjust
//! colour) executed under node against hand-computed values. Skipped when node
//! isn't installed.

#[test]
fn runtime_pixel_stages_compute_what_ac_render_does() {
    if !std::process::Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success()) {
        eprintln!("node not found: skipping");
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = std::process::Command::new("node").arg(dir.join("tests/runtime_stages.js")).arg(dir.join("html5_runtime.js")).output().unwrap();
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
