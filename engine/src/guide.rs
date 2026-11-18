//! A plugin's guide: one page that says what the plugin is for and how to use it, to keep or to hand on.
//!
//! What the guide states as fact comes from the plugin itself: its name, a picture of each page of its screen,
//! every control with its unit, where it starts and the hardware knob that turns it, and its presets. The words
//! that explain it come from the AI, which is asked once and can only answer. They are kept with the plugin in
//! `guide.json`, and are asked for again only when the plugin's controls or presets have changed.
//!
//! Kept words may have come in with a plugin somebody else made, so every word is treated as plain text: it is
//! shortened to what a guide needs, and written into the page with nothing in it able to act as markup.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use ring::digest::{SHA256, digest};
use serde_json::{Value, json};

use crate::agent::{AgentError, Brief, Entry, Model, ModelError, ToolDefinition, ToolInput};
use crate::ai_settings::AiSettings;
use crate::arranged_screen::{ArrangedItem, ArrangedPage, arranged_screen_of};
use crate::claude_code;
use crate::library::{built_parameters, summary_of};
use crate::look::ItemKind;
use crate::page_view::page_views;
use crate::shown_words::{form_written_in, one_line, shortened};
use crate::stop_signal::StopSignal;

/// The words of the guide, kept with the plugin.
pub const WORDS_FILE: &str = "guide.json";
/// The guide as one page, written into the plugin's folder.
pub const PAGE_FILE: &str = "guide.html";

const HOW_TO_WRITE_A_GUIDE: &str = include_str!("../../directives/plugin-guide.md");
const FORM: &str = "write_guide";
const ABOUT_THE_FORM: &str = "Hands over the words of the guide. This is the only thing you can do, so always use it.";
/// Claude Code is given no form to fill in, so it is told to write one out.
const WRITE_THE_FORM_OUT: &str = "\n\nYou have no tools. Answer with the form itself and nothing else, as one JSON object: \
                                  {\"about\": \"...\", \"controls\": [{\"name\": \"...\", \"does\": \"...\"}], \
                                  \"presets\": [{\"name\": \"...\", \"is\": \"...\"}], \"toTry\": [\"...\"]}";
const PAGE: &str = include_str!("guide_page.html");
const LONGEST_ABOUT: usize = 700;
const LONGEST_ENTRY: usize = 420;
const LONGEST_NAME: usize = 64;
const MOST_CONTROLS: usize = 128;
const MOST_PRESETS: usize = 64;
const MOST_THINGS_TO_TRY: usize = 6;
const MOST_CHOICES_LISTED: usize = 12;
const MOST_OF_A_NOTE: usize = 8_000;
const MOST_OF_THE_CODE: usize = 60_000;
const NOTES_BY_THE_MAKER: [(&str, &str); 2] = [("README.md", "What its maker wrote about it"), ("DESIGN.md", "The plan for its sound and its look")];
const CODE_FOLDER: &str = "src";
const CODE_ENDINGS: [&str; 3] = ["cpp", "h", "hpp"];
const WEBSITE: &str = "https://forge-audio.github.io/mp-forge/";

/// One control as the built plugin reports it and as the screen places it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    /// The name the plugin's code and layout know it by.
    pub name: String,
    /// The name shown on the screen, which the musician may have changed.
    pub shown_name: String,
    /// Where it starts, as the plugin words it, with its unit.
    pub starts_at: String,
    /// What it can be set to, when it is a choice.
    pub choices: Vec<String>,
    pub knob: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub name: String,
    /// The page as it is drawn, as a PNG. Empty when it could not be drawn.
    pub picture: Vec<u8>,
    pub controls: Vec<Control>,
}

/// Everything the guide says of a plugin that is read from the plugin itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub name: String,
    pub maker: String,
    pub is_effect: bool,
    pub pages: Vec<Page>,
    pub presets: Vec<String>,
}

/// What is said of one control or one preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub of: String,
    pub words: String,
}

/// The words of a guide: what explains the plugin, as against what is read from it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Words {
    pub about: String,
    pub controls: Vec<Said>,
    pub presets: Vec<Said>,
    pub to_try: Vec<String>,
}

#[derive(Debug)]
pub enum GuideError {
    /// Only a built plugin can say what its controls and presets are.
    NotBuiltYet,
    Ai(AgentError),
    /// The AI answered, but not with a guide.
    NothingWritten,
    Io(io::Error),
}

impl fmt::Display for GuideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GuideError::NotBuiltYet => write!(formatter, "A guide can be made once the plugin has been built. Run its safety check first."),
            GuideError::Ai(error) => error.fmt(formatter),
            GuideError::NothingWritten => write!(formatter, "The AI did not come back with a guide. Ask for it again."),
            GuideError::Io(cause) => write!(formatter, "The guide could not be saved: {cause}"),
        }
    }
}

impl From<AgentError> for GuideError {
    fn from(problem: AgentError) -> Self {
        GuideError::Ai(problem)
    }
}

impl From<ModelError> for GuideError {
    fn from(problem: ModelError) -> Self {
        GuideError::Ai(AgentError::Model(problem))
    }
}

impl From<io::Error> for GuideError {
    fn from(cause: io::Error) -> Self {
        GuideError::Io(cause)
    }
}

// ---------------------------------------------------------------------------
// What is read from the plugin
// ---------------------------------------------------------------------------

/// What the built plugin and its screen say of themselves.
pub fn facts_of(plugin_dir: &Path) -> Result<Facts, GuideError> {
    let built = built_parameters(plugin_dir).ok_or(GuideError::NotBuiltYet)?;
    let summary = summary_of(plugin_dir).ok_or(GuideError::NotBuiltYet)?;
    let reported = built["params"].as_array().map(Vec::as_slice).unwrap_or_default();
    let mut pictures: HashMap<String, Vec<u8>> = page_views(plugin_dir).into_iter().map(|view| (view.page_name, view.png)).collect();
    let page = |arranged: ArrangedPage| Page {
        picture: pictures.remove(&arranged.name).unwrap_or_default(),
        controls: arranged.items.iter().filter(|item| item.kind == ItemKind::Control).map(|item| control_from(item, reported)).collect(),
        name: arranged.name,
    };
    Ok(Facts {
        name: summary.name,
        maker: summary.maker,
        is_effect: summary.kind.eq_ignore_ascii_case("effect"),
        pages: arranged_screen_of(plugin_dir).into_iter().map(page).collect(),
        presets: built["programs"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
    })
}

fn control_from(on_the_screen: &ArrangedItem, reported: &[Value]) -> Control {
    let named = |report: &&Value| report["name"].as_str().is_some_and(|name| name.eq_ignore_ascii_case(&on_the_screen.name));
    let report = reported.iter().find(named).unwrap_or(&Value::Null);
    let words = |field: &str| report[field].as_str().unwrap_or_default().trim().to_string();
    Control {
        name: on_the_screen.name.clone(),
        shown_name: on_the_screen.shown_name.clone().unwrap_or_else(|| on_the_screen.name.clone()),
        starts_at: [words("display"), words("label")].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" "),
        choices: report["choices"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
        knob: on_the_screen.knob,
    }
}

impl Facts {
    fn controls(&self) -> impl Iterator<Item = &Control> {
        self.pages.iter().flat_map(|page| &page.controls)
    }

    /// What the kept words were written for. It changes when a control or a preset is added, taken away or renamed.
    fn fingerprint(&self) -> String {
        let controls: Vec<&str> = self.controls().map(|control| control.name.as_str()).collect();
        let listed = format!("{}\n--\n{}", controls.join("\n"), self.presets.join("\n"));
        digest(&SHA256, listed.as_bytes()).as_ref().iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn kind(&self) -> &'static str {
        if self.is_effect { "effect" } else { "instrument" }
    }
}

// ---------------------------------------------------------------------------
// The words, kept with the plugin
// ---------------------------------------------------------------------------

/// The words kept with the plugin, when they were written for the controls and presets it has now.
pub fn kept_words(plugin_dir: &Path, facts: &Facts) -> Option<Words> {
    let kept: Value = serde_json::from_str(&fs::read_to_string(plugin_dir.join(WORDS_FILE)).ok()?).ok()?;
    let still_fit = kept["writtenFor"].as_str() == Some(facts.fingerprint().as_str());
    still_fit.then(|| words_on(&kept)).filter(Words::say_something)
}

pub fn keep_words(plugin_dir: &Path, facts: &Facts, words: &Words) -> io::Result<()> {
    let said = |said: &[Said], field: &str| said.iter().map(|said| json!({ "name": said.of, field: said.words })).collect::<Vec<Value>>();
    let kept = json!({
        "writtenFor": facts.fingerprint(),
        "about": words.about,
        "controls": said(&words.controls, "does"),
        "presets": said(&words.presets, "is"),
        "toTry": words.to_try,
    });
    fs::write(plugin_dir.join(WORDS_FILE), serde_json::to_string_pretty(&kept).expect("a guide's words are always plain JSON") + "\n")
}

impl Words {
    fn say_something(&self) -> bool {
        !self.about.is_empty() || !self.controls.is_empty()
    }

    fn of_control(&self, name: &str) -> &str {
        words_of(&self.controls, name)
    }

    fn of_preset(&self, name: &str) -> &str {
        words_of(&self.presets, name)
    }
}

fn words_of<'a>(said: &'a [Said], name: &str) -> &'a str {
    said.iter().find(|said| said.of.eq_ignore_ascii_case(name)).map(|said| said.words.as_str()).unwrap_or_default()
}

/// The words on a form, each on one line and no longer than a guide needs. Anything else on the form is left out.
fn words_on(form: &Value) -> Words {
    let entries = |list: &str, field: &str, most: usize| -> Vec<Said> {
        let written = form[list].as_array().map(Vec::as_slice).unwrap_or_default();
        written.iter().filter_map(|entry| said_in(entry, field)).take(most).collect()
    };
    let things_to_try = form["toTry"].as_array().into_iter().flatten().filter_map(Value::as_str);
    Words {
        about: form["about"].as_str().and_then(|about| one_line(about, LONGEST_ABOUT)).unwrap_or_default(),
        controls: entries("controls", "does", MOST_CONTROLS),
        presets: entries("presets", "is", MOST_PRESETS),
        to_try: things_to_try.filter_map(|thing| one_line(thing, LONGEST_ENTRY)).take(MOST_THINGS_TO_TRY).collect(),
    }
}

fn said_in(entry: &Value, field: &str) -> Option<Said> {
    Some(Said { of: one_line(entry["name"].as_str()?, LONGEST_NAME)?, words: one_line(entry[field].as_str()?, LONGEST_ENTRY)? })
}

// ---------------------------------------------------------------------------
// Asking the AI for the words
// ---------------------------------------------------------------------------

/// Asks a model the workbench talks to directly. The model is handed the form and nothing else.
pub fn ask_model(model: &mut dyn Model, material: &str) -> Result<Words, GuideError> {
    let form = [ToolDefinition { name: FORM, description: ABOUT_THE_FORM, input_schema: shape_of_the_form() }];
    let said = [Entry::User(material.to_string())];
    let turn = model.next_turn(&Brief { instructions: HOW_TO_WRITE_A_GUIDE, tools: &form, history: &said })?;
    let filled_in = turn.tool_calls.iter().find(|call| call.name == FORM).and_then(|call| match &call.input {
        ToolInput::Valid(form) => Some(words_on(form)),
        ToolInput::Unparseable(_) => None,
    });
    worth_keeping(filled_in.unwrap_or_else(|| words_written_in(&turn.text)))
}

/// Asks through the musician's own Claude Code, which is given no tools at all for this.
pub fn ask_claude_code(material: &str, settings: AiSettings, stop: &StopSignal) -> Result<Words, GuideError> {
    let instructions = format!("{HOW_TO_WRITE_A_GUIDE}{WRITE_THE_FORM_OUT}");
    worth_keeping(words_written_in(&claude_code::answer_once(&instructions, material, settings, stop)?))
}

fn words_written_in(answer: &str) -> Words {
    form_written_in(answer).map(|form| words_on(&form)).unwrap_or_default()
}

fn worth_keeping(words: Words) -> Result<Words, GuideError> {
    if words.say_something() { Ok(words) } else { Err(GuideError::NothingWritten) }
}

fn shape_of_the_form() -> Value {
    let entry = |field: &str, about: &str| json!({
        "type": "object",
        "properties": { "name": { "type": "string", "description": "Its exact name, as given." }, field: { "type": "string", "description": about } },
        "required": ["name", field],
    });
    json!({
        "type": "object",
        "properties": {
            "about": { "type": "string", "description": "Two or three sentences on what the plugin is and when to reach for it." },
            "controls": { "type": "array", "items": entry("does", "What the control does to the sound, in one or two sentences."), "description": "One entry for every control." },
            "presets": { "type": "array", "items": entry("is", "A few words on what the preset sounds like or is good for."), "description": "One entry for every preset." },
            "toTry": { "type": "array", "items": { "type": "string" }, "description": "Three to five things to try, one sentence each." },
        },
        "required": ["about", "controls", "presets", "toTry"],
    })
}

/// Everything the AI is given to write from: what the plugin has, what its maker wrote, and its code.
pub fn material_for(plugin_dir: &Path, facts: &Facts) -> String {
    let opening = format!("The plugin is called \"{}\". It is an {}.", facts.name, facts.kind());
    let pages: Vec<String> = facts.pages.iter().map(controls_listed).collect();
    let presets = format!("Its presets: {}.", facts.presets.join(", "));
    let notes = NOTES_BY_THE_MAKER.iter().filter_map(|(file, heading)| note_from(plugin_dir, file, heading));
    let parts = [opening, pages.join("\n"), presets].into_iter().chain(notes).chain(std::iter::once(code_of(plugin_dir)));
    parts.collect::<Vec<_>>().join("\n\n")
}

fn controls_listed(page: &Page) -> String {
    let described = |control: &Control| {
        let choices = if control.choices.is_empty() { String::new() } else { format!(", a choice of {}", control.choices.join(" / ")) };
        format!("- {}{} (starts at {})", control.name, choices, control.starts_at)
    };
    format!("The controls on its page {}:\n{}", page.name, page.controls.iter().map(described).collect::<Vec<_>>().join("\n"))
}

fn note_from(plugin_dir: &Path, file: &str, heading: &str) -> Option<String> {
    let written = fs::read_to_string(plugin_dir.join(file)).ok().filter(|written| !written.trim().is_empty())?;
    Some(format!("{heading} ({file}):\n\n{}", shortened(written.trim(), MOST_OF_A_NOTE)))
}

fn code_of(plugin_dir: &Path) -> String {
    let is_code = |file: &PathBuf| file.extension().and_then(|ending| ending.to_str()).is_some_and(|ending| CODE_ENDINGS.contains(&ending));
    let mut files: Vec<PathBuf> = fs::read_dir(plugin_dir.join(CODE_FOLDER)).into_iter().flatten().flatten().map(|entry| entry.path()).filter(is_code).collect();
    files.sort();
    let whole: String = files.iter().filter_map(|file| fs::read_to_string(file).ok()).collect::<Vec<_>>().join("\n\n");
    format!("Its code:\n\n{}", shortened(&whole, MOST_OF_THE_CODE))
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

/// Writes the guide into the plugin's folder as one page that needs nothing beside it, and returns where it is.
pub fn write_page(plugin_dir: &Path, facts: &Facts, words: &Words) -> io::Result<PathBuf> {
    let page_file = plugin_dir.join(PAGE_FILE);
    fs::write(&page_file, page_of(facts, words))?;
    Ok(page_file)
}

/// The guide as one web page. Every word in it is written as plain text, whoever wrote it.
pub fn page_of(facts: &Facts, words: &Words) -> String {
    let screen: Vec<String> = facts.pages.iter().map(|page| page_section(page, facts.pages.len(), words)).collect();
    let filled_in = [
        ("{{NAME}}", plain(&facts.name)),
        ("{{WHAT_IT_IS}}", plain(&format!("An {} for the Akai MPC, by {}", facts.kind(), facts.maker))),
        ("{{ABOUT}}", paragraph(&words.about)),
        ("{{SCREEN}}", screen.join("\n")),
        ("{{PRESETS}}", presets_section(facts, words)),
        ("{{TO_TRY}}", things_to_try_section(words)),
        ("{{ON_YOUR_MPC}}", on_your_mpc(facts)),
        ("{{WEBSITE}}", plain(WEBSITE)),
    ];
    filled_in.iter().fold(PAGE.to_string(), |page, (place, with)| page.replace(place, with))
}

/// Text written so that nothing in it can act as markup, or be taken for one of the page's own places to fill in.
fn plain(text: &str) -> String {
    text.chars().map(|letter| match letter {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#39;".to_string(),
        '{' => "&#123;".to_string(),
        other => other.to_string(),
    }).collect()
}

fn paragraph(words: &str) -> String {
    if words.is_empty() { String::new() } else { format!("<p class=\"about\">{}</p>", plain(words)) }
}

fn page_section(page: &Page, pages_in_all: usize, words: &Words) -> String {
    let heading = if pages_in_all == 1 { "The screen".to_string() } else { format!("The {} page", page.name) };
    let picture = if page.picture.is_empty() { String::new() } else { picture_of(page) };
    let rows: Vec<String> = page.controls.iter().map(|control| control_row(control, words)).collect();
    let table = if rows.is_empty() { String::new() } else { format!("<div class=\"table-holder\"><table><thead><tr><th>Control</th><th>What it does</th><th>Starts at</th><th>Hardware knob</th></tr></thead><tbody>\n{}\n</tbody></table></div>", rows.join("\n")) };
    format!("<section><h2>{}</h2>\n{picture}\n{table}</section>", plain(&heading))
}

fn picture_of(page: &Page) -> String {
    format!("<img alt=\"The {} page of the plugin's screen\" src=\"data:image/png;base64,{}\">", plain(&page.name), BASE64_STANDARD.encode(&page.picture))
}

fn control_row(control: &Control, words: &Words) -> String {
    let knob = control.knob.map(|knob| knob.to_string()).unwrap_or_default();
    format!(
        "<tr><th scope=\"row\">{}</th><td>{}{}</td><td>{}</td><td class=\"knob\">{}</td></tr>",
        plain(&control.shown_name), plain(words.of_control(&control.name)), settings_of(control), plain(&control.starts_at), knob
    )
}

/// What a choice can be set to, when the list is short enough to read.
fn settings_of(control: &Control) -> String {
    match control.choices.len() {
        0 => String::new(),
        few if few <= MOST_CHOICES_LISTED => format!("<div class=\"settings\">Settings: {}</div>", plain(&control.choices.join(", "))),
        many => format!("<div class=\"settings\">{many} settings to choose from.</div>"),
    }
}

fn presets_section(facts: &Facts, words: &Words) -> String {
    if facts.presets.is_empty() {
        return String::new();
    }
    let listed: Vec<String> = facts.presets.iter().map(|preset| format!("<li><b>{}</b> {}</li>", plain(preset), plain(words.of_preset(preset)))).collect();
    format!("<section><h2>Presets</h2><ul class=\"presets\">\n{}\n</ul></section>", listed.join("\n"))
}

fn things_to_try_section(words: &Words) -> String {
    if words.to_try.is_empty() {
        return String::new();
    }
    let listed: Vec<String> = words.to_try.iter().map(|thing| format!("<li>{}</li>", plain(thing))).collect();
    format!("<section><h2>Things to try</h2><ol>\n{}\n</ol></section>", listed.join("\n"))
}

fn on_your_mpc(facts: &Facts) -> String {
    let where_it_goes = if facts.is_effect { "It is an effect, so it goes on an insert." } else { "It is an instrument, so it goes on a plugin track." };
    plain(&format!("On the MPC, open the list of plugins. It is under External Plugins, by {}. {where_it_goes}", facts.maker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::test_support::{ScriptedModel, calls, says};
    use crate::workspace::test_support::TemporaryFolder;

    fn control(name: &str, starts_at: &str, knob: u32) -> Control {
        Control { name: name.to_string(), shown_name: name.to_string(), starts_at: starts_at.to_string(), choices: Vec::new(), knob: Some(knob) }
    }

    fn dusty_tape() -> Facts {
        let mode = Control { choices: vec!["Clean".to_string(), "Worn".to_string()], ..control("Mode", "Worn", 14) };
        let renamed = Control { shown_name: "SPEED".to_string(), ..control("Time", "320 ms", 13) };
        Facts {
            name: "Dusty Tape".to_string(),
            maker: "Forge Tests".to_string(),
            is_effect: true,
            pages: vec![Page { name: "MAIN".to_string(), picture: vec![1, 2, 3], controls: vec![renamed, mode] }],
            presets: vec!["Default".to_string(), "Seasick".to_string()],
        }
    }

    fn written() -> Value {
        json!({
            "about": "A worn tape echo. Reach for it when a part sounds too clean.",
            "controls": [{ "name": "Time", "does": "Sets how far apart the echoes fall." }, { "name": "mode", "does": "Picks how worn the tape is." }, { "name": "Wobble", "does": "Not a control of this plugin." }],
            "presets": [{ "name": "Default", "is": "How the plugin starts." }, { "name": "Seasick", "is": "Deep, slow wobble." }],
            "toTry": ["Choose Seasick and turn Time down slowly.", "Switch Mode to Clean on drums."],
        })
    }

    #[test]
    fn a_model_is_handed_only_the_form_and_the_guides_words_are_read_from_it() {
        let mut model = ScriptedModel::playing([calls(FORM, written())]);

        let words = ask_model(&mut model, "the material").unwrap();

        assert_eq!(words.about, "A worn tape echo. Reach for it when a part sounds too clean.");
        assert_eq!((words.of_control("Time"), words.of_control("Mode")), ("Sets how far apart the echoes fall.", "Picks how worn the tape is."));
        assert_eq!((words.of_preset("Seasick"), words.to_try.len()), ("Deep, slow wobble.", 2));
        assert_eq!(model.histories_seen[0], [Entry::User("the material".to_string())]);
    }

    #[test]
    fn a_guide_written_out_as_an_answer_is_read_too_and_an_answer_with_none_is_refused() {
        let mut writes_it_out = ScriptedModel::playing([says(&format!("Here it is:\n{}", written()))]);
        let mut chats = ScriptedModel::playing([says("I would rather talk about tape machines.")]);

        assert_eq!(ask_model(&mut writes_it_out, "the material").unwrap().controls.len(), 3);
        assert!(matches!(ask_model(&mut chats, "the material"), Err(GuideError::NothingWritten)));
    }

    #[test]
    fn kept_words_are_used_until_the_plugins_controls_or_presets_change() {
        let plugin = TemporaryFolder::create("guide-kept");
        let words = words_on(&written());
        let with_another_preset = Facts { presets: vec!["Default".to_string(), "Seasick".to_string(), "Chewed".to_string()], ..dusty_tape() };

        keep_words(&plugin.0, &dusty_tape(), &words).unwrap();

        assert_eq!(kept_words(&plugin.0, &dusty_tape()), Some(words));
        assert_eq!(kept_words(&plugin.0, &with_another_preset), None);
        assert_eq!(kept_words(&TemporaryFolder::create("guide-none").0, &dusty_tape()), None);
    }

    #[test]
    fn the_page_states_the_plugins_own_facts_beside_the_words() {
        let page = page_of(&dusty_tape(), &words_on(&written()));

        assert!(page.contains("<h1>Dusty Tape</h1>") && page.contains("An effect for the Akai MPC, by Forge Tests"));
        assert!(page.contains("<h2>The screen</h2>") && page.contains("src=\"data:image/png;base64,AQID\""));
        assert!(page.contains("<tr><th scope=\"row\">SPEED</th><td>Sets how far apart the echoes fall.</td><td>320 ms</td><td class=\"knob\">13</td></tr>"), "{page}");
        assert!(page.contains("Picks how worn the tape is.<div class=\"settings\">Settings: Clean, Worn</div>"));
        assert!(page.contains("<li><b>Seasick</b> Deep, slow wobble.</li>") && page.contains("<li>Switch Mode to Clean on drums.</li>"));
        assert!(page.contains("It is an effect, so it goes on an insert.") && !page.contains("Wobble") && !page.contains("{{"));
    }

    #[test]
    fn nothing_written_in_a_guide_can_act_as_markup() {
        let sly = json!({ "about": "<script>alert(1)</script> & more", "controls": [{ "name": "Time", "does": "<img src=x onerror=alert(1)>" }],
                          "presets": [{ "name": "Default", "is": "\"quoted\"" }], "toTry": ["</ol><b>bold</b>"] });
        let named_slyly = Facts { name: "<b>Tape</b>".to_string(), ..dusty_tape() };

        let page = page_of(&named_slyly, &words_on(&sly));

        assert!(!page.contains("<script>alert") && !page.contains("<img src=x") && !page.contains("<b>bold</b>") && !page.contains("<b>Tape</b>"));
        assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; more") && page.contains("&quot;quoted&quot;"));
    }

    #[test]
    fn words_are_kept_short_and_on_one_line_however_they_were_written() {
        let rambling = json!({ "about": format!("One.\n\nTwo. {}", "x".repeat(2000)), "controls": [{ "name": "Time", "does": "a\nb" }, { "name": "", "does": "nameless" }, "not an entry"],
                               "presets": "not a list", "toTry": (0..20).map(|number| format!("Try {number}")).collect::<Vec<_>>() });

        let words = words_on(&rambling);

        assert!(words.about.chars().count() == LONGEST_ABOUT + 1 && words.about.ends_with('\u{2026}') && !words.about.contains('\n'));
        assert_eq!(words.controls, [Said { of: "Time".to_string(), words: "a b".to_string() }]);
        assert_eq!((words.presets.len(), words.to_try.len()), (0, MOST_THINGS_TO_TRY));
    }

    #[test]
    fn the_ai_is_told_what_the_plugin_has_and_shown_its_notes_and_code() {
        let plugin = TemporaryFolder::create("guide-material");
        fs::create_dir_all(plugin.0.join("src")).unwrap();
        fs::write(plugin.0.join("README.md"), "A worn tape echo.").unwrap();
        fs::write(plugin.0.join("src/DustyTape.cpp"), "// wobble = slow sine on the delay time").unwrap();
        fs::write(plugin.0.join("src/notes.txt"), "not code").unwrap();

        let material = material_for(&plugin.0, &dusty_tape());

        assert!(material.starts_with("The plugin is called \"Dusty Tape\". It is an effect."));
        assert!(material.contains("The controls on its page MAIN:\n- Time (starts at 320 ms)\n- Mode, a choice of Clean / Worn (starts at Worn)"));
        assert!(material.contains("Its presets: Default, Seasick."));
        assert!(material.contains("What its maker wrote about it (README.md):\n\nA worn tape echo."));
        assert!(material.contains("// wobble = slow sine on the delay time") && !material.contains("not code") && !material.contains("DESIGN.md"));
    }

    #[test]
    fn a_plugin_that_has_not_been_built_has_no_guide_yet() {
        let plugin = TemporaryFolder::create("guide-unbuilt");

        assert!(matches!(facts_of(&plugin.0), Err(GuideError::NotBuiltYet)));
    }
}
