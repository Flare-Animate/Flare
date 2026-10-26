//! Port of `stdfx/igs_resource_sleep.cpp` (both win and unix variants):
//! sleep for a given number of milliseconds.

use std::thread;
use std::time::Duration;

/// Sleep for `milli_seconds` milliseconds.
pub fn sleep_m(milli_seconds: u32) {
    thread::sleep(Duration::from_millis(milli_seconds as u64));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_does_not_panic() {
        sleep_m(0);
        sleep_m(1);
    }
}