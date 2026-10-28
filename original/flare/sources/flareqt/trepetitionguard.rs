//! Port of `flareqt/trepetitionguard.cpp`: a re-entrancy guard that tracks
//! how many `TRepetitionGuard` instances are live. `has_lock()` is true for
//! the first one created while no other is alive.

use std::sync::atomic::{AtomicI32, Ordering};

static COUNTER: AtomicI32 = AtomicI32::new(0);

#[derive(Debug)]
pub struct TRepetitionGuard {
    counter_check: bool,
}

impl TRepetitionGuard {
    pub fn new() -> Self {
        let counter_check = COUNTER.load(Ordering::SeqCst) == 0;
        COUNTER.fetch_add(1, Ordering::SeqCst);
        Self { counter_check }
    }

    pub fn has_lock(&self) -> bool {
        self.counter_check
    }
}

impl Drop for TRepetitionGuard {
    fn drop(&mut self) {
        COUNTER.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Default for TRepetitionGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_instance_has_lock() {
        let g = TRepetitionGuard::new();
        assert!(g.has_lock());
    }

    #[test]
    fn nested_instance_does_not_have_lock() {
        let g1 = TRepetitionGuard::new();
        assert!(g1.has_lock());
        {
            let g2 = TRepetitionGuard::new();
            assert!(!g2.has_lock());
        }
        // g1 is still alive, so a new guard does NOT have the lock
        let g3 = TRepetitionGuard::new();
        assert!(!g3.has_lock());
        drop(g3);
        drop(g1);
        // Now that all are gone, a new guard has the lock
        let g4 = TRepetitionGuard::new();
        assert!(g4.has_lock());
    }
}