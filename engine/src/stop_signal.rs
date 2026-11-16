//! A way for the musician to stop the AI part-way through its work.
//!
//! Whoever starts a job keeps one copy of the signal and hands another to the job. The job looks at it
//! between steps and gives up as soon as it has been asked to stop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default)]
pub struct StopSignal(Arc<AtomicBool>);

impl StopSignal {
    pub fn ask_to_stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_asked(&self) -> bool {
        self.0.load(Ordering::SeqCst)
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
}
