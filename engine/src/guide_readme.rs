//! A plugin's guide as a README: the page a plugin's own repository opens on.
//!
//! It says what the guide's page says, from the same facts and the same words, written as Markdown so that the
//! place the repository is kept shows it as a page. The pictures of the screen are files beside it. A plugin whose
//! guide has not been written still gets its pictures and its controls, without the words on what each one does.

use crate::guide::{Control, Facts, MOST_CHOICES_LISTED, Page, WEBSITE, Words};

const SAYS_WHAT_EACH_DOES: &str = "What it does";
/// A plugin whose guide has not been written can still say what a choice can be set to.
const SAYS_ONLY_THE_SETTINGS: &str = "Settings";
const MEANS_SOMETHING_IN_MARKDOWN: &str = "\\`*_[]#|~";

/// Where the picture of a page is kept beside the README, counting pages from one.
pub fn picture_file_of(page_number: usize) -> String {
    format!("docs/screen-{page_number}.png")
}

/// The README. `about` is what is said of the plugin when its guide has no words on that yet.
pub fn readme_of(facts: &Facts, words: &Words, about: &str) -> String {
    let said_about = if words.about.is_empty() { about } else { &words.about };
    let pages = facts.pages.iter().enumerate().map(|(index, page)| page_section(facts, page, index + 1, words));
    let sections = [
        format!("# {}\n\n{}.\n\n{}", plain(&facts.name), plain(&facts.what_it_is()), plain(said_about)),
        pages.collect::<Vec<String>>().join("\n\n"),
        presets_section(facts, words),
        things_to_try_section(words),
        format!("## On your MPC\n\n{}", plain(&facts.where_it_is_on_the_mpc())),
        format!("Made with [MP Forge]({WEBSITE})."),
    ];
    sections.into_iter().filter(|section| !section.is_empty()).collect::<Vec<String>>().join("\n\n") + "\n"
}

fn page_section(facts: &Facts, page: &Page, page_number: usize, words: &Words) -> String {
    let heading = if facts.pages.len() == 1 { "The screen".to_string() } else { format!("The {} page", plain(&page.name)) };
    let picture = if page.picture.is_empty() { String::new() } else { format!("\n\n![The {} page of the plugin's screen]({})", plain(&page.name), picture_file_of(page_number)) };
    let rows: Vec<String> = page.controls.iter().map(|control| control_row(control, words)).collect();
    let table = if rows.is_empty() { String::new() } else { format!("\n\n{}\n{}", table_heading(words), rows.join("\n")) };
    format!("## {heading}{picture}{table}")
}

fn table_heading(words: &Words) -> String {
    let middle = if words.controls.is_empty() { SAYS_ONLY_THE_SETTINGS } else { SAYS_WHAT_EACH_DOES };
    format!("| Control | {middle} | Starts at | Hardware knob |\n|---|---|---|---|")
}

fn control_row(control: &Control, words: &Words) -> String {
    let knob = control.knob.map(|knob| knob.to_string()).unwrap_or_default();
    let what_it_does = [plain(words.of_control(&control.name)), settings_of(control)].join(" ");
    format!("| **{}** | {} | {} | {knob} |", plain(&control.shown_name), what_it_does.trim(), plain(&control.starts_at))
}

/// What a choice can be set to, when the list is short enough to read.
fn settings_of(control: &Control) -> String {
    match control.choices.len() {
        0 => String::new(),
        few if few <= MOST_CHOICES_LISTED => format!("Settings: {}.", plain(&control.choices.join(", "))),
        many => format!("{many} settings to choose from."),
    }
}

fn presets_section(facts: &Facts, words: &Words) -> String {
    if facts.presets.is_empty() {
        return String::new();
    }
    let listed: Vec<String> = facts.presets.iter().map(|preset| format!("- **{}** {}", plain(preset), plain(words.of_preset(preset))).trim_end().to_string()).collect();
    format!("## Presets\n\n{}", listed.join("\n"))
}

fn things_to_try_section(words: &Words) -> String {
    if words.to_try.is_empty() {
        return String::new();
    }
    let listed: Vec<String> = words.to_try.iter().enumerate().map(|(index, thing)| format!("{}. {}", index + 1, plain(thing))).collect();
    format!("## Things to try\n\n{}", listed.join("\n"))
}

/// Text written on one line so that nothing in it is taken for Markdown or for markup, whoever wrote it.
fn plain(text: &str) -> String {
    let on_one_line = text.split_whitespace().collect::<Vec<&str>>().join(" ");
    on_one_line.chars().map(|letter| match letter {
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '&' => "&amp;".to_string(),
        means_something if MEANS_SOMETHING_IN_MARKDOWN.contains(means_something) => format!("\\{means_something}"),
        other => other.to_string(),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::Said;

    fn control(name: &str, starts_at: &str, knob: u32) -> Control {
        Control { name: name.to_string(), shown_name: name.to_string(), starts_at: starts_at.to_string(), choices: Vec::new(), knob: Some(knob) }
    }

    fn tape_echo() -> Facts {
        let mode = Control { choices: vec!["Clean".to_string(), "Worn".to_string()], ..control("Mode", "Clean", 2) };
        let main = Page { name: "MAIN".to_string(), picture: b"png".to_vec(), controls: vec![control("Time", "350 ms", 1), mode] };
        let more = Page { name: "MORE".to_string(), picture: Vec::new(), controls: Vec::new() };
        Facts { name: "Tape Echo".to_string(), maker: "Somebody".to_string(), is_effect: true, pages: vec![main, more], presets: vec!["Default".to_string()] }
    }

    fn its_words() -> Words {
        let said = |of: &str, words: &str| Said { of: of.to_string(), words: words.to_string() };
        Words {
            about: "A worn tape echo.".to_string(),
            controls: vec![said("Time", "How long before the sound comes back.")],
            presets: vec![said("Default", "How the plugin starts.")],
            to_try: vec!["Turn Time up slowly.".to_string()],
        }
    }

    #[test]
    fn the_readme_says_what_the_guide_says_with_the_pictures_beside_it() {
        let readme = readme_of(&tape_echo(), &its_words(), "An echo.");

        assert!(readme.starts_with("# Tape Echo\n\nAn effect for the Akai MPC, by Somebody.\n\nA worn tape echo.\n\n## The MAIN page\n\n![The MAIN page of the plugin's screen](docs/screen-1.png)\n\n| Control |"));
        assert!(readme.contains("| **Time** | How long before the sound comes back. | 350 ms | 1 |"));
        assert!(readme.contains("| **Mode** | Settings: Clean, Worn. | Clean | 2 |"));
        assert!(readme.contains("## The MORE page\n\n## Presets\n\n- **Default** How the plugin starts.\n\n## Things to try\n\n1. Turn Time up slowly.\n\n## On your MPC\n\n"));
        assert!(readme.ends_with("Made with [MP Forge](https://forge-audio.github.io/mp-forge/).\n"));
    }

    #[test]
    fn a_plugin_without_a_guide_still_gets_its_controls_and_what_was_said_about_it() {
        let readme = readme_of(&tape_echo(), &Words::default(), "An echo with a worn sound.");

        assert!(readme.contains("by Somebody.\n\nAn echo with a worn sound.\n\n## The MAIN page"));
        assert!(readme.contains("| Control | Settings | Starts at | Hardware knob |") && !readme.contains("What it does"));
        assert!(readme.contains("| **Time** |  | 350 ms | 1 |"));
        assert!(readme.contains("- **Default**\n") && !readme.contains("Things to try"));
    }

    #[test]
    fn nothing_written_in_a_readme_can_act_as_markdown_or_markup() {
        let mut words = its_words();
        words.about = "See [this](http://example.com) <script>alert(1)</script>\n# Heading | cell".to_string();

        let readme = readme_of(&tape_echo(), &words, "");

        assert!(readme.contains("See \\[this\\](http://example.com) &lt;script&gt;alert(1)&lt;/script&gt; \\# Heading \\| cell"));
    }
}
