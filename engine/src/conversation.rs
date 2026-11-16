//! What the musician and the AI have said about one plugin, kept in the plugin's folder.
//!
//! The screen writes the conversation here so it is still there after the app is closed. Each new
//! request to the AI carries the last few messages, so the AI knows what "I mean the other one" refers to.
//! The AI can neither see nor write this folder itself.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

const FOLDER: &str = ".workbench";
const FILE: &str = "conversation.json";
const MOST_MESSAGES_KEPT: usize = 200;
const MESSAGES_RECALLED: usize = 12;
const LONGEST_MESSAGE_RECALLED: usize = 600;
/// Who said a message, by the kind the screen gives it, in the words the AI is told.
const SPEAKERS: [(&str, &str); 3] = [("you", "Musician"), ("said", "You"), ("note", "MP Forge")];

/// The messages kept for a plugin, oldest first. A plugin with none, or with a file that cannot be read, has none.
pub fn kept(plugin_dir: &Path) -> Vec<Value> {
    let written = fs::read_to_string(file_of(plugin_dir)).unwrap_or_default();
    serde_json::from_str::<Vec<Value>>(&written).unwrap_or_default()
}

/// Keeps the conversation as it now stands. Only the newest messages are kept once it grows long.
pub fn keep(plugin_dir: &Path, messages: &[Value]) -> io::Result<()> {
    let newest = &messages[messages.len().saturating_sub(MOST_MESSAGES_KEPT)..];
    fs::create_dir_all(plugin_dir.join(FOLDER))?;
    fs::write(file_of(plugin_dir), Value::from(newest.to_vec()).to_string())
}

/// The last few messages, worded for the AI to read before a new request. Empty when nothing was said before.
pub fn earlier_talk(plugin_dir: &Path) -> String {
    let said: Vec<String> = kept(plugin_dir).iter().filter_map(line_of).collect();
    let recalled = &said[said.len().saturating_sub(MESSAGES_RECALLED)..];
    if recalled.is_empty() {
        return String::new();
    }
    format!(
        "\n\nWhat was said about this plugin before this request, oldest first. It is here so you know what the \
         musician is referring to. Only the request above is to be acted on.\n\n{}",
        recalled.join("\n")
    )
}

fn file_of(plugin_dir: &Path) -> PathBuf {
    plugin_dir.join(FOLDER).join(FILE)
}

fn line_of(message: &Value) -> Option<String> {
    let kind = message["kind"].as_str()?;
    let speaker = SPEAKERS.iter().find(|(known, _)| *known == kind).map(|(_, speaker)| speaker)?;
    let words: String = message["text"].as_str()?.trim().chars().take(LONGEST_MESSAGE_RECALLED).collect();
    Some(format!("{speaker}: {}", words.replace('\n', " ")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn message(kind: &str, text: &str) -> Value {
        json!({ "kind": kind, "text": text })
    }

    #[test]
    fn a_kept_conversation_reads_back_as_it_was_written() {
        let plugin = TemporaryFolder::create("conversation");
        let messages = [message("you", "make it warmer"), message("said", "I lowered the tone.")];

        keep(&plugin.0, &messages).unwrap();

        assert_eq!(kept(&plugin.0), messages);
        assert!(plugin.0.join(".workbench/conversation.json").is_file());
    }

    #[test]
    fn a_plugin_nothing_was_said_about_has_no_conversation_and_no_earlier_talk() {
        let plugin = TemporaryFolder::create("conversation-none");

        assert!(kept(&plugin.0).is_empty());
        assert_eq!(earlier_talk(&plugin.0), "");
    }

    #[test]
    fn earlier_talk_names_who_spoke_and_leaves_out_problems() {
        let plugin = TemporaryFolder::create("conversation-talk");
        let messages = [
            message("you", "put the controls on top"),
            message("said", "I moved them\nto the top two rows."),
            message("problem", "Not safe to send yet."),
            message("note", "Went back to the version before the last change."),
        ];
        keep(&plugin.0, &messages).unwrap();

        let talk = earlier_talk(&plugin.0);

        assert!(talk.contains("Musician: put the controls on top\nYou: I moved them to the top two rows.\nMP Forge: Went back"), "{talk}");
        assert!(!talk.contains("Not safe"));
    }

    #[test]
    fn only_the_newest_messages_are_kept_and_fewer_still_are_recalled() {
        let plugin = TemporaryFolder::create("conversation-long");
        let messages: Vec<Value> = (1..=MOST_MESSAGES_KEPT + 5).map(|number| message("you", &format!("request {number}"))).collect();

        keep(&plugin.0, &messages).unwrap();

        let talk = earlier_talk(&plugin.0);
        assert_eq!(kept(&plugin.0).len(), MOST_MESSAGES_KEPT);
        assert_eq!(talk.matches("Musician:").count(), MESSAGES_RECALLED);
        assert!(talk.ends_with(&format!("request {}", MOST_MESSAGES_KEPT + 5)));
    }
}
