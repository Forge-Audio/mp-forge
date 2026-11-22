//! Where a plugin came from, when it is somebody else's work taken in from the Open MPC catalog: what it is
//! called there, who made it, where its source is kept and the licence it is shared under.
//!
//! It is written once, when the plugin is taken in, and travels with the plugin's project from then on. The
//! licence decides what the musician owes when they share their own version, so a listing reads it from here.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Value, json};

use crate::shown_words::{one_line, secure_web_address};

pub const ORIGIN_FILE: &str = "origin.json";
const LONGEST_WORD: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub name: String,
    pub author: String,
    /// The page its source is kept on.
    pub source: String,
    pub license: String,
}

impl Origin {
    /// Where the plugin in this folder came from. None for a plugin that was made here, and for a note that
    /// does not say all four things plainly.
    pub fn of_plugin(plugin_dir: &Path) -> Option<Origin> {
        let written = fs::read_to_string(plugin_dir.join(ORIGIN_FILE)).ok()?;
        Origin::said_in(&serde_json::from_str(&written).ok()?)
    }

    pub fn said_in(note: &Value) -> Option<Origin> {
        let word = |field: &str| note[field].as_str().and_then(|written| one_line(written, LONGEST_WORD));
        Some(Origin { name: word("name")?, author: word("author")?, source: secure_web_address(&word("source")?)?, license: word("license")? })
    }

    pub fn write_into(&self, plugin_dir: &Path) -> io::Result<()> {
        let note = json!({ "name": self.name, "author": self.author, "source": self.source, "license": self.license });
        fs::write(plugin_dir.join(ORIGIN_FILE), serde_json::to_string_pretty(&note).expect("a note is always plain JSON") + "\n")
    }

    /// The original and its maker in one sentence, to stand wherever the musician's version is shared.
    pub fn credit(&self) -> String {
        format!("Based on {} by {} ({}), which is shared under {}.", self.name, self.author, self.source, self.license)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn tape_echo() -> Origin {
        Origin { name: "Tape Echo".to_string(), author: "somebody".to_string(), source: "https://github.com/somebody/mpc-tape-echo".to_string(), license: "GPL-3.0-only".to_string() }
    }

    #[test]
    fn where_a_plugin_came_from_is_read_back_as_it_was_written() {
        let plugin = TemporaryFolder::create("origin-kept");

        tape_echo().write_into(&plugin.0).unwrap();

        assert_eq!(Origin::of_plugin(&plugin.0), Some(tape_echo()));
        assert_eq!(tape_echo().credit(), "Based on Tape Echo by somebody (https://github.com/somebody/mpc-tape-echo), which is shared under GPL-3.0-only.");
    }

    #[test]
    fn a_plugin_made_here_has_no_origin_and_a_note_that_leaves_something_out_is_not_one() {
        let plugin = TemporaryFolder::create("origin-none");

        assert_eq!(Origin::of_plugin(&plugin.0), None);
        assert_eq!(Origin::said_in(&json!({ "name": "Tape Echo", "author": "somebody", "source": "https://github.com/somebody/mpc-tape-echo" })), None);
        assert_eq!(Origin::said_in(&json!({ "name": "Tape Echo", "author": "somebody", "source": "javascript:alert(1)", "license": "MIT" })), None);
    }
}
