//! The card the musician picked for their MPC Sample, kept here between the picking and the writing.
//!
//! Where it is never passes through the screen. So an app is only ever written where the musician pointed in
//! this computer's own chooser, whatever the page asks for.

use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct PickedCard {
    picked: Mutex<Option<PathBuf>>,
}

impl PickedCard {
    pub fn keep(&self, picked: PathBuf) {
        *self.held() = Some(picked);
    }

    /// The card that was picked, which is then no longer kept: each writing takes a picking of its own.
    pub fn take(&self) -> Option<PathBuf> {
        self.held().take()
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Option<PathBuf>> {
        self.picked.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_card_is_written_to_once_and_then_has_to_be_picked_again() {
        let picked = PickedCard::default();

        picked.keep(PathBuf::from("/Volumes/MPC SAMPLE"));

        assert_eq!(picked.take(), Some(PathBuf::from("/Volumes/MPC SAMPLE")));
        assert_eq!(picked.take(), None);
    }
}
