//! Words an AI wrote, made fit to show to the musician.

/// Words fit to show on one line of the screen, or nothing when there are none.
pub fn one_line(words: &str, longest: usize) -> Option<String> {
    let on_one_line = words.split_whitespace().collect::<Vec<_>>().join(" ");
    (!on_one_line.is_empty()).then(|| shortened(&on_one_line, longest))
}

pub fn shortened(words: &str, longest: usize) -> String {
    let words = words.trim();
    if words.chars().count() <= longest { words.to_string() } else { format!("{}\u{2026}", words.chars().take(longest).collect::<String>()) }
}

/// The form in an answer that is the form written out as JSON, perhaps with other words round it.
pub fn form_written_in(answer: &str) -> Option<serde_json::Value> {
    let form = answer.find('{').zip(answer.rfind('}')).filter(|(start, end)| start < end).map(|(start, end)| &answer[start..=end]);
    form.and_then(|written| serde_json::from_str(written).ok())
}

/// A web address somebody else wrote, passed on only when it is the plain address of a secure page.
pub fn secure_web_address(written: &str) -> Option<String> {
    const LONGEST_ADDRESS: usize = 300;
    let address = written.trim();
    let is_plain = address.starts_with("https://") && address.len() <= LONGEST_ADDRESS && !address.contains(char::is_whitespace);
    is_plain.then(|| address.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_secure_address_is_passed_on() {
        assert_eq!(secure_web_address(" https://example.org/a "), Some("https://example.org/a".to_string()));
        assert_eq!(secure_web_address("http://example.org/a"), None);
        assert_eq!(secure_web_address("javascript:alert(1)"), None);
        assert_eq!(secure_web_address("https://example.org/a b"), None);
    }

    #[test]
    fn words_are_put_on_one_line_and_cut_where_they_run_long() {
        assert_eq!(one_line("  a worn\n tape  echo ", 40).as_deref(), Some("a worn tape echo"));
        assert_eq!(one_line("a worn tape echo", 6).as_deref(), Some("a worn\u{2026}"));
        assert_eq!(one_line(" \n ", 40), None);
    }

    #[test]
    fn a_form_is_found_among_other_words() {
        assert_eq!(form_written_in("Here it is: {\"name\": \"Tide\"} I hope it helps."), Some(serde_json::json!({ "name": "Tide" })));
        assert_eq!(form_written_in("No form here."), None);
    }
}
