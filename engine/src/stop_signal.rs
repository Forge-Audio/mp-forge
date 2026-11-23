//! A way for the musician to stop the AI part-way through its work.
//!
//! Whoever starts a job keeps one copy of the signal and hands another to the job. The job looks at it
//! between steps and gives up as soon as it has been asked to stop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const LOOKS_AT_THE_SIGNAL_EVERY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Default)]
pub struct StopSignal(Arc<AtomicBool>);

impl StopSignal {
    pub fn ask_to_stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_asked(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Waits this long, or until it is asked to stop. Says whether the whole wait passed.
    pub fn lets_pass(&self, wait: Duration) -> bool {
        let ends = Instant::now() + wait;
        while !self.is_asked() {
            let left = ends.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return true;
            }
            thread::sleep(left.min(LOOKS_AT_THE_SIGNAL_EVERY));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_copy_of_a_signal_hears_the_same_request() {
        let kept = StopSignal::default();
        let handed_to_the_job = kept.clone();

        assert!(!handed_to_the_job.is_asked());
        kept.ask_to_stop();

        assert!(handed_to_the_job.is_asked());
        assert!(!StopSignal::default().is_asked());
    }

    #[test]
    fn a_wait_passes_whole_unless_the_signal_asks_to_stop() {
        let asked = StopSignal::default();
        asked.ask_to_stop();

        assert!(StopSignal::default().lets_pass(Duration::from_millis(5)));
        assert!(!asked.lets_pass(Duration::from_secs(3600)));
    }
}
