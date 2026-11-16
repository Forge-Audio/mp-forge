//! Finding words in the text of files, and changing a few of them, so the AI need not read or rewrite a whole
//! file to find or fix one thing in it.

const MOST_LINES_FOUND: usize = 60;
const LONGEST_LINE_SHOWN: usize = 200;

/// Every line that holds the words looked for, whatever their capitals, as `path:line: the line`. `files` are the
/// paths to look in with the text of each. Only the first lines found are shown, and the answer says when
/// there were more.
pub fn lines_holding(wanted: &str, files: impl Iterator<Item = (String, String)>) -> String {
    let wanted = wanted.to_lowercase();
    let mut found: Vec<String> = files.flat_map(|(path, text)| lines_in_one_file(&wanted, &path, &text)).collect();
    if found.is_empty() {
        return "Nothing found.".to_string();
    }
    let all_of_them = found.len();
    found.truncate(MOST_LINES_FOUND);
    if all_of_them > MOST_LINES_FOUND {
        found.push(format!("… and {} more lines. Search for something narrower.", all_of_them - MOST_LINES_FOUND));
    }
    found.join("\n")
}

fn lines_in_one_file(wanted_in_small_letters: &str, path: &str, text: &str) -> Vec<String> {
    let holds_it = |(_, line): &(usize, &str)| line.to_lowercase().contains(wanted_in_small_letters);
    text.lines().enumerate().filter(holds_it).map(|(number, line)| format!("{path}:{}: {}", number + 1, shown(line))).collect()
}

fn shown(line: &str) -> String {
    let line = line.trim();
    if line.chars().count() <= LONGEST_LINE_SHOWN { line.to_string() } else { format!("{}…", line.chars().take(LONGEST_LINE_SHOWN).collect::<String>()) }
}

/// The text with one passage swapped for another, and how many places were changed. The passage has to be
/// there, and there only once unless every place is to change, so that nothing is changed by mistake.
pub fn with_passage_replaced(text: &str, passage: &str, replacement: &str, everywhere: bool) -> Result<(String, usize), String> {
    if passage.is_empty() {
        return Err("Give the passage to find. It cannot be empty.".to_string());
    }
    match text.matches(passage).count() {
        0 => Err("That passage is not in the file as you gave it. Read the file again and copy the passage exactly, spaces and line breaks included.".to_string()),
        1 => Ok((text.replacen(passage, replacement, 1), 1)),
        places if everywhere => Ok((text.replace(passage, replacement), places)),
        places => Err(format!("That passage is in the file {places} times. Give more of what surrounds the one you mean, or set `everywhere` to change them all.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_files() -> impl Iterator<Item = (String, String)> {
        let files = [("src/Echo.cpp", "float mix = 0.5f;\n// The Mix knob\nfloat time;"), ("ui.json", "{ \"param\": \"Mix\" }")];
        files.into_iter().map(|(path, text)| (path.to_string(), text.to_string()))
    }

    #[test]
    fn lines_are_found_whatever_their_capitals_and_named_by_file_and_line() {
        let found = lines_holding("MIX", two_files());

        assert_eq!(found, "src/Echo.cpp:1: float mix = 0.5f;\nsrc/Echo.cpp:2: // The Mix knob\nui.json:1: { \"param\": \"Mix\" }");
    }

    #[test]
    fn finding_nothing_is_said_plainly() {
        assert_eq!(lines_holding("reverb", two_files()), "Nothing found.");
    }

    #[test]
    fn only_the_first_lines_are_shown_when_there_are_many() {
        let long_file = "knob\n".repeat(MOST_LINES_FOUND + 5);

        let found = lines_holding("knob", [("a.cpp".to_string(), long_file)].into_iter());

        assert_eq!(found.lines().count(), MOST_LINES_FOUND + 1);
        assert!(found.ends_with("… and 5 more lines. Search for something narrower."));
    }

    #[test]
    fn a_passage_found_once_is_replaced() {
        let changed = with_passage_replaced("fill=\"#ff0000\" r=\"4\"", "#ff0000", "#00c2a8", false);

        assert_eq!(changed, Ok(("fill=\"#00c2a8\" r=\"4\"".to_string(), 1)));
    }

    #[test]
    fn a_passage_that_is_missing_or_found_twice_changes_nothing() {
        let missing = with_passage_replaced("abc", "x", "y", false).unwrap_err();
        let twice = with_passage_replaced("red red", "red", "blue", false).unwrap_err();

        assert!(missing.contains("not in the file"));
        assert!(twice.contains("2 times"));
    }

    #[test]
    fn every_place_changes_when_asked() {
        assert_eq!(with_passage_replaced("red red", "red", "blue", true), Ok(("blue blue".to_string(), 2)));
    }
}
