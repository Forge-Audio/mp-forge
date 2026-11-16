//! How the musician's other plugins look, told to the AI when it starts a new one so the new screen is unlike them.
//!
//! Only what makes one screen look like another is told: the idea behind it, the kit its controls come from,
//! the colour of its ground and the fonts of its lettering.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::layers::Art;
use crate::library::{self, PluginSummary};
use crate::look::{self, Look};

const DESIGN_PLAN: &str = "DESIGN.md";
const MOST_PLUGINS_TOLD_OF: usize = 8;
const LONGEST_IDEA: usize = 200;
const HEADING_MARK: char = '#';
const HEADING_OF_THE_LOOK: &str = "look";
const MARKS_LEFT_OUT_OF_AN_IDEA: [char; 2] = ['*', '`'];
const ENDS_OF_A_SENTENCE: [char; 4] = ['.', '!', '?', '\u{2026}'];
/// A colour in `ui.json` is written as eight hexadecimal digits, the first two saying how solid it is.
const DIGITS_OF_HOW_SOLID: usize = 2;

/// What the AI is told about the looks there are already. Says nothing when this is the musician's first plugin.
pub fn note_on_other_looks(plugins_dir: &Path, new_plugin: &str) -> String {
    let others = library::list_plugins(plugins_dir).into_iter().filter(|plugin| plugin.name != new_plugin);
    let told: Vec<String> = others.take(MOST_PLUGINS_TOLD_OF).map(|plugin| line_about(plugins_dir, &plugin)).collect();
    if told.is_empty() {
        return String::new();
    }
    format!("\n\nHow the musician's other plugins look. Make this one's screen clearly unlike each of them:\n\n{}", told.join("\n"))
}

fn line_about(plugins_dir: &Path, plugin: &PluginSummary) -> String {
    let plugin_dir = plugins_dir.join(&plugin.name);
    let said: Vec<String> = [idea_of(&plugin_dir), controls_and_ground_of(&plugin_dir), lettering_of(&plugin_dir)].into_iter().flatten().collect();
    if said.is_empty() { format!("- {}: a plain screen.", plugin.name) } else { format!("- {}: {}", plugin.name, said.join(" ")) }
}

/// The idea of the look as its plan puts it: the first line of writing under the heading about the look, or the
/// first in the whole plan when no heading speaks of the look.
fn idea_of(plugin_dir: &Path) -> Option<String> {
    let plan = fs::read_to_string(plugin_dir.join(DESIGN_PLAN)).ok()?;
    let from_the_look_on = plan.lines().skip_while(|line| !is_heading_of_the_look(line)).skip(1);
    let idea = first_writing_in(from_the_look_on).or_else(|| first_writing_in(plan.lines()))?;
    Some(ended_as_a_sentence(shortened(&idea)))
}

fn ended_as_a_sentence(words: String) -> String {
    if words.ends_with(ENDS_OF_A_SENTENCE) { words } else { format!("{words}.") }
}

fn is_heading_of_the_look(line: &str) -> bool {
    line.starts_with(HEADING_MARK) && line.to_lowercase().contains(HEADING_OF_THE_LOOK)
}

fn first_writing_in<'plan>(lines: impl Iterator<Item = &'plan str>) -> Option<String> {
    let written = lines.map(|line| line.replace(MARKS_LEFT_OUT_OF_AN_IDEA, ""));
    written.map(|line| line.trim().to_string()).find(|line| !line.is_empty() && !line.starts_with(HEADING_MARK))
}

fn shortened(idea: &str) -> String {
    let kept: String = idea.chars().take(LONGEST_IDEA).collect();
    if kept.len() < idea.len() { format!("{}\u{2026}", kept.trim_end()) } else { kept }
}

fn controls_and_ground_of(plugin_dir: &Path) -> Option<String> {
    let layout = library::screen_layout(plugin_dir)?;
    let kit = name_of_the_kit(&layout["kit"]).map(|kit| format!("The {kit} kit"));
    let ground = layout["style"]["background"].as_str().and_then(as_a_web_colour).map(|colour| format!("a {colour} ground"));
    match (kit, ground) {
        (Some(kit), Some(ground)) => Some(format!("{kit} on {ground}.")),
        (Some(kit), None) => Some(format!("{kit}.")),
        (None, Some(ground)) => Some(format!("Controls on {ground}.")),
        (None, None) => None,
    }
}

/// A kit is written as its name alone, or with a colour of its own beside the name.
fn name_of_the_kit(kit: &Value) -> Option<String> {
    let colour_of_its_own = kit["accent"].as_str().and_then(as_a_web_colour);
    match (kit.as_str().or(kit["theme"].as_str()), colour_of_its_own) {
        (Some(name), Some(colour)) => Some(format!("{name} (in {colour})")),
        (name, _) => name.map(str::to_string),
    }
}

fn as_a_web_colour(written: &str) -> Option<String> {
    let colour = written.get(DIGITS_OF_HOW_SOLID..)?;
    (!colour.is_empty()).then(|| format!("#{colour}"))
}

fn lettering_of(plugin_dir: &Path) -> Option<String> {
    let fonts = fonts_lettered_in(&look::look_of(plugin_dir).ok()?);
    (!fonts.is_empty()).then(|| format!("Lettered in {}.", fonts.into_iter().collect::<Vec<_>>().join(" and ")))
}

/// The fonts of the words drawn as artwork, each once, in the order of the alphabet.
fn fonts_lettered_in(look: &Look) -> BTreeSet<String> {
    let layers = look.pages.iter().flat_map(|page| &page.artwork);
    layers.filter_map(|layer| match &layer.art {
        Art::Words(words) => Some(words.font.clone()),
        _ => None,
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn plugin_in(plugins: &TemporaryFolder, name: &str) -> std::path::PathBuf {
        let plugin_dir = plugins.0.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.json"), format!(r#"{{ "name": "{name}", "manufacturer": "Workbench Tests", "kind": "effect" }}"#)).unwrap();
        plugin_dir
    }

    const WORDS_IN_FUTURA: &str = r##"{ "pages": [ { "page": "MAIN", "artwork": [
        { "id": 1, "kind": "words", "text": "ECHO", "font": "Futura", "size": 40, "colour": "#ffffff", "place": { "x": 10, "y": 10 } },
        { "id": 2, "kind": "words", "text": "TONE", "font": "Futura", "size": 20, "colour": "#ffffff", "place": { "x": 10, "y": 80 } }
    ] } ] }"##;

    #[test]
    fn a_first_plugin_is_told_of_no_other_looks() {
        let plugins = TemporaryFolder::create("other-looks-none");
        plugin_in(&plugins, "Brand New");

        assert_eq!(note_on_other_looks(&plugins.0, "Brand New"), "");
    }

    #[test]
    fn each_other_plugin_is_told_by_its_idea_its_kit_its_ground_and_its_fonts() {
        let plugins = TemporaryFolder::create("other-looks-told");
        let tape = plugin_in(&plugins, "Dusty Tape");
        fs::write(tape.join("DESIGN.md"), "# Dusty Tape\n\n## The sound\nA delay that sinks.\n\n## The look\n**Idea:** the front of a worn tape machine.\n- Olive ground\n").unwrap();
        fs::write(tape.join("ui.json"), r#"{ "kit": "amber", "pages": [], "style": { "background": "ff151813" } }"#).unwrap();
        fs::write(tape.join("look.json"), WORDS_IN_FUTURA).unwrap();
        plugin_in(&plugins, "Brand New");

        let note = note_on_other_looks(&plugins.0, "Brand New");

        assert!(note.contains("Make this one's screen clearly unlike each of them"), "{note}");
        assert!(note.ends_with("- Dusty Tape: Idea: the front of a worn tape machine. The amber kit on a #151813 ground. Lettered in Futura."), "{note}");
        assert!(!note.contains("Brand New"), "{note}");
    }

    #[test]
    fn a_kit_given_a_colour_of_its_own_is_told_with_that_colour() {
        let plugins = TemporaryFolder::create("other-looks-own-colour");
        let glass = plugin_in(&plugins, "Glass Pad");
        fs::write(glass.join("ui.json"), r#"{ "kit": { "theme": "mono", "accent": "ffd08a2e" }, "pages": [] }"#).unwrap();

        assert!(note_on_other_looks(&plugins.0, "Another").ends_with("- Glass Pad: The mono (in #d08a2e) kit."));
    }

    #[test]
    fn a_plan_with_no_heading_for_the_look_gives_its_first_line_of_writing() {
        let plugins = TemporaryFolder::create("other-looks-old-plan");
        let sampler = plugin_in(&plugins, "Bit Box");
        fs::write(sampler.join("DESIGN.md"), "# Bit Box\n\n## The idea in one sentence\nA beige desktop sampler from the 1990s.\n").unwrap();

        assert!(note_on_other_looks(&plugins.0, "Another").ends_with("- Bit Box: A beige desktop sampler from the 1990s."));
    }

    #[test]
    fn a_plugin_with_nothing_styled_is_said_to_be_plain() {
        let plugins = TemporaryFolder::create("other-looks-plain");
        plugin_in(&plugins, "Bare One");

        assert!(note_on_other_looks(&plugins.0, "Another").ends_with("- Bare One: a plain screen."));
    }

    #[test]
    fn a_long_idea_is_cut_short_and_only_so_many_plugins_are_told_of() {
        let plugins = TemporaryFolder::create("other-looks-many");
        for number in 0..MOST_PLUGINS_TOLD_OF + 3 {
            let plugin_dir = plugin_in(&plugins, &format!("Plugin {number:02}"));
            fs::write(plugin_dir.join("DESIGN.md"), format!("## The look\n{}\n", "very ".repeat(100))).unwrap();
        }

        let note = note_on_other_looks(&plugins.0, "Another");

        assert_eq!(note.matches("\n- Plugin").count(), MOST_PLUGINS_TOLD_OF);
        assert!(note.lines().filter(|line| line.starts_with("- ")).all(|line| line.chars().count() < LONGEST_IDEA + 40), "{note}");
    }
}
