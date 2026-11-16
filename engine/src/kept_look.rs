//! Keeping a look with its plugin, once it is known to fit the plugin's screen.
//!
//! The musician's editor and the AI's styling tool both keep a look through here, so neither can keep
//! one the MPC's screen cannot show.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use crate::arranged_screen::{self, ArrangedPage, arranged};
use crate::look::{self, Look};
use crate::page_controls;
use crate::pictures::pictures_of;
use crate::screen_layout::{SCREEN_LAYOUT, ScreenLayout};

#[derive(Debug)]
pub enum NotKept {
    /// Why the look does not fit the screen, in words for whoever asked for it.
    DoesNotFit(Vec<String>),
    CouldNotBeWritten(io::Error),
}

impl fmt::Display for NotKept {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NotKept::DoesNotFit(problems) => write!(formatter, "{}", problems.join(" ")),
            NotKept::CouldNotBeWritten(error) => write!(formatter, "The look could not be saved: {error}"),
        }
    }
}

impl From<io::Error> for NotKept {
    fn from(error: io::Error) -> Self {
        NotKept::CouldNotBeWritten(error)
    }
}

/// Keeps a look with its plugin. Controls put on a page or taken off one since the look before are
/// written into the screen's layout as well. Returns the screen as it is now arranged.
/// A look that does not fit is refused, and then nothing at all is written.
pub fn keep_if_it_fits(plugin_dir: &Path, look: &Look) -> Result<Vec<ArrangedPage>, NotKept> {
    let layout_file = plugin_dir.join(SCREEN_LAYOUT);
    let layout_before = fs::read_to_string(&layout_file).unwrap_or_default();
    let look_before = look::look_of(plugin_dir).unwrap_or_default();
    let layout_after = page_controls::with_changes_between(&layout_before, &look_before, look).map_err(|problem| NotKept::DoesNotFit(vec![problem]))?;
    let pages = arranged(&ScreenLayout::written_in(&layout_after), &pictures_of(plugin_dir).unwrap_or_default(), look);
    let problems = arranged_screen::problems_with(&pages);
    if !problems.is_empty() {
        return Err(NotKept::DoesNotFit(problems));
    }
    if layout_after != layout_before {
        fs::write(&layout_file, layout_after)?;
    }
    look::keep_look(plugin_dir, look)?;
    Ok(pages)
}
