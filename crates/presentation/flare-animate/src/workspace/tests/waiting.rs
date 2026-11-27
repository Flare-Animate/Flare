//! Waiting on background work in a test, as a running app does between frames.

use super::{Workspace, run};

/// Runs UI passes until `done` holds or fifteen seconds pass (a font job takes about three in a debug build, more
/// on a busy machine), the way a running app would.
pub(super) fn wait_until(
    ws: &mut Workspace,
    ctx: &egui::Context,
    done: impl Fn(&Workspace) -> bool,
) {
    let started = std::time::Instant::now();
    let mut time = 100.0;
    while !done(ws) {
        assert!(
            started.elapsed().as_secs() < 15,
            "timed out waiting for background work"
        );
        run(ws, ctx, time);
        time += 0.016;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
