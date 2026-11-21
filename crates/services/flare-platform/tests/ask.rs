//! A file picker runs off the thread that asked, so the window never waits for it.
#![allow(clippy::expect_used)]

use std::sync::mpsc;
use std::time::Duration;

use flare_platform::jobs;

#[cfg(not(target_os = "macos"))]
#[test]
fn a_picker_blocks_its_own_thread_not_the_caller() {
    let (release, hold) = mpsc::channel::<()>();
    let (answered, answer) = mpsc::channel();
    let caller = std::thread::current().id();
    jobs::ask(
        move || {
            // The picker waits for the person, here for the test to let it go.
            hold.recv_timeout(Duration::from_secs(20)).ok()?;
            Some(std::thread::current().id())
        },
        move |picked| {
            let _ = answered.send(picked);
        },
    )
    .expect("starts");
    // `ask` returned while the picker still waits: the caller was free.
    assert!(answer.try_recv().is_err());
    release.send(()).expect("release");
    let picked = answer
        .recv_timeout(Duration::from_secs(20))
        .expect("answered")
        .expect("a pick");
    assert_ne!(picked, caller);
}

#[test]
fn a_cancelled_picker_answers_none() {
    let (answered, answer) = mpsc::channel();
    jobs::ask(
        || None::<u32>,
        move |picked| {
            let _ = answered.send(picked);
        },
    )
    .expect("starts");
    assert_eq!(
        answer
            .recv_timeout(Duration::from_secs(20))
            .expect("answered"),
        None
    );
}
