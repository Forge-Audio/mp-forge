//! The six knobs an effect is played with on an MPC Sample.
//!
//! The MPC Sample shows none of a plugin's pages. It loads an effect as a Knob FX: three knobs, and the same
//! three while SHIFT is held, each with a short label. Which control sits on which knob is kept with the
//! plugin in `sample-knobs.json`, by the control's name, so it stays right when the code gains a control.
//!
//! The MPC Sample reads the knobs from the built plugin's `plugin.json`, by the control's number. So after
//! every build the names are looked up in what the built plugin reports, and the knobs are written there.
//! An effect that names no knobs gets a first guess, so that none leaves with the knobs the MPC Sample falls
//! back to, which start at the control every MPC nudges when it loads a plugin.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::artwork;
use crate::library;
use crate::ordered_json::Json;
use crate::picture_clock::CLOCK_CONTROL;
use crate::screen_layout::{ScreenLayout, SlotContent};
use crate::sdk::built_plugin_folder;

/// Where the six knobs are kept, with the plugin.
pub const SIX_KNOBS_FILE: &str = "sample-knobs.json";
pub const KNOBS_IN_A_ROW: usize = 3;
/// The MPC Sample shows a short name as it is written and never cuts it.
pub const LONGEST_SHORT_NAME: usize = 12;
/// Three labels share a screen 320 pixels wide.
pub const LONGEST_LABEL: usize = 9;

const SHORT_NAME: &str = "shortName";
const PLAIN_KNOBS: &str = "knobs";
const SHIFT_KNOBS: &str = "shiftKnobs";
const SAID_FOR_THE_MPC_SAMPLE: [&str; 3] = [SHORT_NAME, PLAIN_KNOBS, SHIFT_KNOBS];
const BY_NAME: &str = "control";
const BY_NUMBER: &str = "parameter";
const LABEL: &str = "label";
const MANIFEST: &str = "plugin.json";
/// Letters, digits and the punctuation a label or a short name is written with.
const PUNCTUATION_IN_LABELS: &str = " -+/%#&.'";

/// What a knob turns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Turns {
    #[default]
    Nothing,
    /// One of the plugin's controls, by the name its code gives it.
    Control(String),
    /// A control by its number in the built plugin, as a plugin written by hand with Akai's SDK names it.
    Numbered(u32),
}

/// One knob of the MPC Sample. Without a label of its own it is labelled with the name of what it turns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Knob {
    pub turns: Turns,
    pub label: String,
}

pub type RowOfKnobs = [Knob; KNOBS_IN_A_ROW];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SixKnobs {
    /// The effect's name where the MPC Sample has little room. Empty for the start of the plugin's own name.
    pub short_name: String,
    pub plain: RowOfKnobs,
    pub with_shift: RowOfKnobs,
}

impl SixKnobs {
    /// The knobs kept with the plugin. None when it keeps none, and what is wrong with them when they cannot
    /// be followed.
    pub fn kept_with(plugin: &Path) -> Result<Option<SixKnobs>, Vec<String>> {
        match fs::read_to_string(plugin.join(SIX_KNOBS_FILE)) {
            Ok(written) => SixKnobs::written_in(&written).map(Some),
            Err(_) => Ok(None),
        }
    }

    pub fn written_in(knobs_file: &str) -> Result<SixKnobs, Vec<String>> {
        let said: Value = serde_json::from_str(knobs_file).map_err(|error| vec![format!("{SIX_KNOBS_FILE} cannot be read: {error}")])?;
        SixKnobs::said_in(&said).ok_or_else(|| problems_in(&said))
    }

    /// The knobs a `plugin.json` names, for a plugin written by hand with Akai's SDK. None when it names none
    /// or names them in a way that cannot be followed.
    pub fn named_in_a_manifest(manifest: &Value) -> Option<SixKnobs> {
        let names_some = SAID_FOR_THE_MPC_SAMPLE.iter().any(|key| manifest.get(key).is_some());
        if names_some { SixKnobs::said_in(manifest) } else { None }
    }

    fn said_in(said: &Value) -> Option<SixKnobs> {
        problems_in(said).is_empty().then(|| SixKnobs {
            short_name: said[SHORT_NAME].as_str().unwrap_or_default().to_string(),
            plain: row_said_in(&said[PLAIN_KNOBS]),
            with_shift: row_said_in(&said[SHIFT_KNOBS]),
        })
    }

    /// As it is kept with the plugin, and as it is handed to whoever shows it.
    pub fn as_written(&self) -> Value {
        let mut said = Map::new();
        if !self.short_name.is_empty() {
            said.insert(SHORT_NAME.to_string(), json!(self.short_name));
        }
        said.insert(PLAIN_KNOBS.to_string(), row_as_kept(&self.plain));
        said.insert(SHIFT_KNOBS.to_string(), row_as_kept(&self.with_shift));
        Value::Object(said)
    }

    pub fn as_a_file(&self) -> String {
        serde_json::to_string_pretty(&self.as_written()).expect("knobs are always plain JSON") + "\n"
    }

    /// The same knobs with nothing left to work out: every control by its name, every label and the short
    /// name as the MPC Sample would show them.
    fn spelled_out(&self, plugin_name: &str, control_names: &[String]) -> SixKnobs {
        let spell = |knob: &Knob| knob_spelled_out(knob, control_names);
        SixKnobs { short_name: self.short_name_for(plugin_name), plain: self.plain.each_ref().map(spell), with_shift: self.with_shift.each_ref().map(spell) }
    }

    pub fn keep_with(&self, plugin: &Path) -> std::io::Result<()> {
        fs::write(plugin.join(SIX_KNOBS_FILE), self.as_a_file())
    }

    /// The six controls a musician is most likely to reach for, when nobody has chosen: the controls on the
    /// plugin's pages in the order they are shown, or failing pages every control it has. The control the MPC
    /// nudges at every load, which is nearly always the volume, comes last.
    pub fn first_guess(control_names: &[String], layout: &ScreenLayout) -> SixKnobs {
        let mut likely = controls_shown_on(layout, control_names);
        if likely.is_empty() {
            likely = control_names.to_vec();
        }
        likely.retain(|control| !control.eq_ignore_ascii_case(CLOCK_CONTROL));
        if likely.len() > 1 && artwork::number_of(&likely[0], control_names) == Some(0) {
            likely.rotate_left(1);
        }
        let mut guessed = likely.into_iter().map(|control| Knob { turns: Turns::Control(control), label: String::new() });
        SixKnobs { short_name: String::new(), plain: row_from(&mut guessed), with_shift: row_from(&mut guessed) }
    }

    /// The knobs as the MPC Sample reads them from a built plugin's `plugin.json`: each control by its number
    /// among `control_names`, each with a label that fits.
    pub fn for_the_mpc_sample(&self, plugin_name: &str, control_names: &[String]) -> Result<Value, Vec<String>> {
        let (plain, with_shift) = (row_by_number(&self.plain, "Knob", control_names), row_by_number(&self.with_shift, "SHIFT knob", control_names));
        let problems: Vec<String> = plain.iter().chain(&with_shift).filter_map(|knob| knob.clone().err()).collect();
        if !problems.is_empty() {
            return Err(problems);
        }
        let read_by_the_mpc = |row: Vec<Result<Value, String>>| Value::Array(row.into_iter().flatten().collect());
        Ok(json!({ SHORT_NAME: self.short_name_for(plugin_name), PLAIN_KNOBS: read_by_the_mpc(plain), SHIFT_KNOBS: read_by_the_mpc(with_shift) }))
    }

    pub fn short_name_for(&self, plugin_name: &str) -> String {
        if self.short_name.is_empty() { cut_to(plugin_name, LONGEST_SHORT_NAME) } else { self.short_name.clone() }
    }
}

/// The knobs the plugin is played with on an MPC Sample: the ones kept with it, or a first guess from what
/// was built of it.
pub fn of_plugin(plugin: &Path) -> Result<SixKnobs, Vec<String>> {
    let guessed = || SixKnobs::first_guess(&artwork::built_control_names(plugin), &ScreenLayout::of_plugin(plugin));
    Ok(SixKnobs::kept_with(plugin)?.unwrap_or_else(guessed))
}

/// The knobs as a musician is shown them, with what there is to choose from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownToTheMusician {
    pub knobs: SixKnobs,
    /// Nobody has chosen yet: these are the ones MP Forge would send the effect out with.
    pub is_a_first_guess: bool,
    /// The plugin's controls as it was last built, by name. Empty until it has been built.
    pub controls: Vec<String>,
}

pub fn shown_for(plugin: &Path) -> Result<ShownToTheMusician, Vec<String>> {
    let control_names = artwork::built_control_names(plugin);
    let plugin_name = library::summary_of(plugin).map(|summary| summary.name).unwrap_or_default();
    let kept = SixKnobs::kept_with(plugin)?;
    let knobs = kept.clone().unwrap_or_else(|| SixKnobs::first_guess(&control_names, &ScreenLayout::of_plugin(plugin)));
    let controls = control_names.iter().filter(|control| !control.eq_ignore_ascii_case(CLOCK_CONTROL)).cloned().collect();
    Ok(ShownToTheMusician { knobs: knobs.spelled_out(&plugin_name, &control_names), is_a_first_guess: kept.is_none(), controls })
}

/// Keeps the knobs a musician chose by hand, once they are knobs the MPC Sample can follow. A plugin that
/// was built is asked whether it has every control they turn.
pub fn keep_chosen(plugin: &Path, chosen: &Value) -> Result<(), Vec<String>> {
    let knobs = SixKnobs::said_in(chosen).ok_or_else(|| problems_in(chosen))?;
    let control_names = artwork::built_control_names(plugin);
    if !control_names.is_empty() {
        knobs.for_the_mpc_sample("", &control_names)?;
    }
    knobs.keep_with(plugin).map_err(|error| vec![format!("The knobs could not be kept: {error}")])
}

/// Writes the knobs into the `plugin.json` of the finished plugin folder the build left behind.
pub fn write_into_the_built_plugin(plugin: &Path) -> Result<(), Vec<String>> {
    let not_built = || vec!["The build left no finished plugin to write the MPC Sample's knobs into.".to_string()];
    let manifest_file = built_plugin_folder(plugin).ok_or_else(not_built)?.join(MANIFEST);
    let mut manifest = fs::read_to_string(&manifest_file).ok().and_then(|written| Json::parse(&written).ok()).ok_or_else(not_built)?;
    let plugin_name = manifest.field("name").and_then(Json::as_text).unwrap_or_default().to_string();
    let for_the_mpc_sample = of_plugin(plugin)?.for_the_mpc_sample(&plugin_name, &artwork::built_control_names(plugin))?;
    for key in SAID_FOR_THE_MPC_SAMPLE {
        manifest.set(key, in_the_order_written(&for_the_mpc_sample[key]));
    }
    fs::write(&manifest_file, manifest.written()).map_err(|error| vec![format!("The MPC Sample's knobs could not be written into the built plugin: {error}")])
}

/// A plugin's own `plugin.json` names no knobs in MP Forge. Akai's starter for an effect names three, by
/// number, for the controls the starter has. Left there they would point at whatever the plugin's code has
/// at those numbers by now, and once it has fewer controls they stop the build.
pub fn take_out_of_the_manifest(plugin: &Path) -> std::io::Result<()> {
    let manifest_file = plugin.join(MANIFEST);
    let Some(Json::Object(mut said)) = fs::read_to_string(&manifest_file).ok().and_then(|written| Json::parse(&written).ok()) else { return Ok(()) };
    let names_no_knobs = said.iter().all(|(key, _)| !SAID_FOR_THE_MPC_SAMPLE.contains(&key.as_str()));
    if names_no_knobs {
        return Ok(());
    }
    said.retain(|(key, _)| !SAID_FOR_THE_MPC_SAMPLE.contains(&key.as_str()));
    fs::write(manifest_file, Json::Object(said).written())
}

fn in_the_order_written(said: &Value) -> Json {
    serde_json::from_value(said.clone()).expect("what serde_json wrote, it reads")
}

fn controls_shown_on(layout: &ScreenLayout, control_names: &[String]) -> Vec<String> {
    let mut shown: Vec<String> = Vec::new();
    for content in layout.pages.iter().flat_map(|page| page.slots()) {
        let SlotContent::Control(control) = content else { continue };
        let is_new = !shown.iter().any(|seen| seen.eq_ignore_ascii_case(control));
        if is_new && artwork::number_of(control, control_names).is_some() {
            shown.push(control.clone());
        }
    }
    shown
}

fn row_from(knobs: &mut impl Iterator<Item = Knob>) -> RowOfKnobs {
    std::array::from_fn(|_| knobs.next().unwrap_or_default())
}

fn row_said_in(said: &Value) -> RowOfKnobs {
    let mut knobs = said.as_array().into_iter().flatten().map(knob_said_in);
    row_from(&mut knobs)
}

fn knob_said_in(said: &Value) -> Knob {
    let by_name = said[BY_NAME].as_str().map(|control| Turns::Control(control.to_string()));
    let by_number = said[BY_NUMBER].as_u64().and_then(|number| u32::try_from(number).ok()).map(Turns::Numbered);
    Knob { turns: by_name.or(by_number).unwrap_or_default(), label: said[LABEL].as_str().unwrap_or_default().to_string() }
}

fn row_as_kept(row: &RowOfKnobs) -> Value {
    Value::Array(row.iter().map(knob_as_kept).collect())
}

fn knob_as_kept(knob: &Knob) -> Value {
    let mut said = Map::new();
    match &knob.turns {
        Turns::Nothing => return json!({}),
        Turns::Control(control) => said.insert(BY_NAME.to_string(), json!(control)),
        Turns::Numbered(number) => said.insert(BY_NUMBER.to_string(), json!(number)),
    };
    if !knob.label.is_empty() {
        said.insert(LABEL.to_string(), json!(knob.label));
    }
    Value::Object(said)
}

fn row_by_number(row: &RowOfKnobs, called: &str, control_names: &[String]) -> Vec<Result<Value, String>> {
    row.iter().enumerate().map(|(position, knob)| knob_by_number(knob, &format!("{called} {}", position + 1), control_names)).collect()
}

fn knob_by_number(knob: &Knob, called: &str, control_names: &[String]) -> Result<Value, String> {
    let number = match &knob.turns {
        Turns::Nothing => return Ok(json!({})),
        Turns::Control(control) => artwork::number_of(control, control_names).ok_or_else(|| no_such_control(called, control, control_names))?,
        Turns::Numbered(number) => *number,
    };
    let name = control_names.get(number as usize).ok_or_else(|| no_such_number(called, number, control_names))?;
    let label = if knob.label.is_empty() { cut_to(name, LONGEST_LABEL) } else { knob.label.clone() };
    Ok(json!({ BY_NUMBER: number, LABEL: label }))
}

fn knob_spelled_out(knob: &Knob, control_names: &[String]) -> Knob {
    let control = match &knob.turns {
        Turns::Nothing => return Knob::default(),
        Turns::Control(control) => control.clone(),
        Turns::Numbered(number) => control_names.get(*number as usize).cloned().unwrap_or_default(),
    };
    let label = if knob.label.is_empty() { cut_to(&control, LONGEST_LABEL) } else { knob.label.clone() };
    Knob { turns: Turns::Control(control), label }
}

fn no_such_control(called: &str, control: &str, control_names: &[String]) -> String {
    format!("{called} of the MPC Sample turns `{control}`, but the plugin's controls are: {}. Change it in {SIX_KNOBS_FILE}.", control_names.join(", "))
}

fn no_such_number(called: &str, number: u32, control_names: &[String]) -> String {
    format!("{called} of the MPC Sample turns control number {number}, but the plugin has {} controls, numbered from 0. Choose its knobs again.", control_names.len())
}

fn cut_to(words: &str, longest: usize) -> String {
    words.chars().take(longest).collect::<String>().trim().to_string()
}

/// What in the knobs as they are written cannot be followed, in words for whoever wrote them.
fn problems_in(said: &Value) -> Vec<String> {
    if !said.is_object() {
        return vec![format!("{SIX_KNOBS_FILE} has to be an object with `{PLAIN_KNOBS}` and `{SHIFT_KNOBS}`.")];
    }
    let mut problems = problems_in_the_short_name(&said[SHORT_NAME]);
    problems.extend(problems_in_a_row(&said[PLAIN_KNOBS], PLAIN_KNOBS));
    problems.extend(problems_in_a_row(&said[SHIFT_KNOBS], SHIFT_KNOBS));
    problems
}

fn problems_in_the_short_name(said: &Value) -> Vec<String> {
    let fits = match said {
        Value::Null => true,
        Value::String(name) => !name.trim().is_empty() && is_written_plainly(name, LONGEST_SHORT_NAME),
        _ => false,
    };
    if fits { Vec::new() } else { vec![format!("`{SHORT_NAME}` has to be a name of at most {LONGEST_SHORT_NAME} plain letters. The MPC Sample shows it as written and never cuts it.")] }
}

fn problems_in_a_row(said: &Value, row: &str) -> Vec<String> {
    match said {
        Value::Null => Vec::new(),
        Value::Array(knobs) if knobs.len() <= KNOBS_IN_A_ROW => knobs.iter().enumerate().filter_map(|(position, knob)| problem_in_a_knob(knob, &format!("`{row}` entry {}", position + 1))).collect(),
        _ => vec![format!("`{row}` has to be a list of at most {KNOBS_IN_A_ROW} knobs: the MPC Sample has three.")],
    }
}

fn problem_in_a_knob(said: &Value, called: &str) -> Option<String> {
    let names_a_control = said[BY_NAME].as_str().is_some_and(|control| !control.is_empty());
    let numbers_a_control = said[BY_NUMBER].as_u64().is_some_and(|number| u32::try_from(number).is_ok());
    let turns_one_thing_or_nothing = match (said.get(BY_NAME), said.get(BY_NUMBER)) {
        (None, None) => true,
        (Some(_), None) => names_a_control,
        (None, Some(_)) => numbers_a_control,
        (Some(_), Some(_)) => false,
    };
    let label_fits = match &said[LABEL] {
        Value::Null => true,
        Value::String(label) => is_written_plainly(label, LONGEST_LABEL),
        _ => false,
    };
    if !said.is_object() || !turns_one_thing_or_nothing {
        Some(format!("{called} has to be {{}} for a knob that turns nothing, or name one control: {{ \"{BY_NAME}\": \"Time\", \"{LABEL}\": \"Time\" }}."))
    } else if !label_fits {
        Some(format!("{called}: its label has to be at most {LONGEST_LABEL} plain letters, because three labels share the MPC Sample's small screen."))
    } else {
        None
    }
}

fn is_written_plainly(words: &str, longest: usize) -> bool {
    let plain = |letter: char| letter.is_ascii_alphanumeric() || PUNCTUATION_IN_LABELS.contains(letter);
    words.chars().count() <= longest && words.chars().all(plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    const DELAY_KNOBS: &str = r#"{ "shortName": "St Delay",
        "knobs": [{ "control": "Time", "label": "Time" }, { "control": "feedback" }, {}],
        "shiftKnobs": [{ "control": "Mix", "label": "Wet" }] }"#;

    fn controls_of_a_delay() -> Vec<String> {
        ["Volume", "Time", "Feedback", "Mix", "Low Cut Frequency"].map(str::to_string).to_vec()
    }

    fn turning(control: &str, label: &str) -> Knob {
        Knob { turns: Turns::Control(control.to_string()), label: label.to_string() }
    }

    fn why_not(written: &str) -> String {
        SixKnobs::written_in(written).unwrap_err().join(" ")
    }

    #[test]
    fn the_knobs_are_read_as_they_are_written_with_empty_seats_left_empty() {
        let knobs = SixKnobs::written_in(DELAY_KNOBS).unwrap();

        assert_eq!(knobs.short_name, "St Delay");
        assert_eq!(knobs.plain, [turning("Time", "Time"), turning("feedback", ""), Knob::default()]);
        assert_eq!(knobs.with_shift, [turning("Mix", "Wet"), Knob::default(), Knob::default()]);
    }

    #[test]
    fn what_is_kept_reads_back_the_same() {
        let knobs = SixKnobs::written_in(DELAY_KNOBS).unwrap();

        assert_eq!(SixKnobs::written_in(&knobs.as_a_file()).unwrap(), knobs);
    }

    #[test]
    fn each_control_is_looked_up_by_name_and_given_its_number_in_the_built_plugin() {
        let knobs = SixKnobs::written_in(DELAY_KNOBS).unwrap();

        let for_the_mpc = knobs.for_the_mpc_sample("Stereo Delay", &controls_of_a_delay()).unwrap();

        assert_eq!(for_the_mpc, json!({
            "shortName": "St Delay",
            "knobs": [{ "parameter": 1, "label": "Time" }, { "parameter": 2, "label": "Feedback" }, {}],
            "shiftKnobs": [{ "parameter": 3, "label": "Wet" }, {}, {}],
        }));
    }

    #[test]
    fn a_knob_without_a_label_and_an_effect_without_a_short_name_get_the_start_of_the_name() {
        let knobs = SixKnobs { plain: [turning("Low Cut Frequency", ""), Knob::default(), Knob::default()], ..SixKnobs::default() };

        let for_the_mpc = knobs.for_the_mpc_sample("A Very Long Effect Name", &controls_of_a_delay()).unwrap();

        assert_eq!(for_the_mpc["knobs"][0], json!({ "parameter": 4, "label": "Low Cut F" }));
        assert_eq!(for_the_mpc["shortName"], "A Very Long");
    }

    #[test]
    fn a_control_that_is_gone_is_said_in_words_with_the_controls_there_are() {
        let knobs = SixKnobs { plain: [turning("Wobble", "Wobble"), Knob::default(), Knob { turns: Turns::Numbered(9), label: String::new() }], ..SixKnobs::default() };

        let problems = knobs.for_the_mpc_sample("Stereo Delay", &controls_of_a_delay()).unwrap_err();

        assert_eq!(problems.len(), 2);
        assert!(problems[0].contains("Knob 1 of the MPC Sample turns `Wobble`") && problems[0].contains("Volume, Time, Feedback, Mix"), "{problems:?}");
        assert!(problems[1].contains("Knob 3 of the MPC Sample turns control number 9, but the plugin has 5 controls"), "{problems:?}");
    }

    #[test]
    fn knobs_that_cannot_be_followed_are_refused_in_words() {
        assert!(why_not("not json").contains("sample-knobs.json cannot be read"));
        assert!(why_not(r#"{ "knobs": [{}, {}, {}, {}] }"#).contains("at most 3 knobs"));
        assert!(why_not(r#"{ "knobs": [{ "control": "Time", "parameter": 1 }] }"#).contains("name one control"));
        assert!(why_not(r#"{ "knobs": [{ "parameter": "1" }] }"#).contains("`knobs` entry 1"));
        assert!(why_not(r#"{ "shiftKnobs": [{ "control": "Time", "label": "Far too long a label" }] }"#).contains("`shiftKnobs` entry 1: its label"));
        assert!(why_not(r#"{ "shortName": "Thirteen chars" }"#).contains("at most 12 plain letters"));
        assert!(why_not(r#"{ "shortName": "<b>" }"#).contains("plain letters"));
    }

    #[test]
    fn a_first_guess_takes_the_controls_in_the_order_the_pages_show_them_with_the_volume_last() {
        let layout = ScreenLayout::written_in(r#"{ "pages": [{ "name": "Main", "controls": ["Volume", "Mix", "Time"] }, { "name": "Tone", "controls": ["Feedback", "Picture Clock", "Mix"] }] }"#);

        let guessed = SixKnobs::first_guess(&controls_of_a_delay(), &layout);

        assert_eq!(guessed.plain, [turning("Mix", ""), turning("Time", ""), turning("Feedback", "")]);
        assert_eq!(guessed.with_shift, [turning("Volume", ""), Knob::default(), Knob::default()]);
    }

    #[test]
    fn a_first_guess_for_a_plugin_without_pages_takes_its_controls_in_order() {
        let guessed = SixKnobs::first_guess(&controls_of_a_delay(), &ScreenLayout::written_in("{}"));

        assert_eq!(guessed.plain, [turning("Time", ""), turning("Feedback", ""), turning("Mix", "")]);
        assert_eq!(guessed.with_shift, [turning("Low Cut Frequency", ""), turning("Volume", ""), Knob::default()]);
    }

    #[test]
    fn the_knobs_a_handwritten_manifest_names_are_carried_by_number() {
        let manifest = json!({ "name": "Stereo Delay", "shortName": "St Delay", "knobs": [{ "parameter": 2, "label": "Time" }, {}, { "parameter": 7, "label": "Mix" }] });

        let knobs = SixKnobs::named_in_a_manifest(&manifest).unwrap();

        assert_eq!(knobs.plain[0], Knob { turns: Turns::Numbered(2), label: "Time".to_string() });
        assert_eq!(knobs.plain[2].turns, Turns::Numbered(7));
        assert_eq!(SixKnobs::named_in_a_manifest(&json!({ "name": "Stereo Delay" })), None);
        assert_eq!(SixKnobs::named_in_a_manifest(&json!({ "knobs": "all of them" })), None);
    }

    #[test]
    fn the_knobs_are_written_into_the_built_plugin_and_everything_else_in_it_is_left_alone() {
        let plugin = TemporaryFolder::create("six-knobs-built");
        let built = plugin.0.join("build/Tests - VST - Stereo Delay");
        fs::create_dir_all(&built).unwrap();
        fs::write(built.join(MANIFEST), r#"{ "name": "Stereo Delay", "kind": "effect", "uid": "TsDl" }"#).unwrap();
        fs::write(plugin.0.join("build/params.json"), json!({ "params": controls_of_a_delay().iter().map(|name| json!({ "name": name })).collect::<Vec<_>>() }).to_string()).unwrap();
        fs::write(plugin.0.join(SIX_KNOBS_FILE), DELAY_KNOBS).unwrap();

        write_into_the_built_plugin(&plugin.0).unwrap();

        let manifest: Value = serde_json::from_str(&fs::read_to_string(built.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!((&manifest["uid"], &manifest["shortName"]), (&json!("TsDl"), &json!("St Delay")));
        assert_eq!(manifest["knobs"][0], json!({ "parameter": 1, "label": "Time" }));
    }

    #[test]
    fn a_musician_is_shown_a_first_guess_spelled_out_until_they_choose_and_then_what_they_chose() {
        let plugin = TemporaryFolder::create("six-knobs-shown");
        fs::create_dir_all(plugin.0.join("build")).unwrap();
        fs::write(plugin.0.join(MANIFEST), r#"{ "name": "Stereo Delay", "manufacturer": "Tests", "kind": "effect" }"#).unwrap();
        fs::write(plugin.0.join("build/params.json"), json!({ "params": controls_of_a_delay().iter().map(|name| json!({ "name": name })).collect::<Vec<_>>() }).to_string()).unwrap();

        let guessed = shown_for(&plugin.0).unwrap();
        keep_chosen(&plugin.0, &json!({ "shortName": "Echo", "knobs": [{ "control": "Mix", "label": "Wet" }] })).unwrap();
        let chosen = shown_for(&plugin.0).unwrap();

        assert!(guessed.is_a_first_guess && !chosen.is_a_first_guess);
        assert_eq!((guessed.knobs.short_name.as_str(), &guessed.knobs.plain[0]), ("Stereo Delay", &turning("Time", "Time")));
        assert_eq!(guessed.controls, controls_of_a_delay());
        assert_eq!((chosen.knobs.short_name.as_str(), &chosen.knobs.plain), ("Echo", &[turning("Mix", "Wet"), Knob::default(), Knob::default()]));
    }

    #[test]
    fn knobs_chosen_by_hand_are_not_kept_when_the_mpc_sample_could_not_follow_them() {
        let plugin = TemporaryFolder::create("six-knobs-refused");
        fs::create_dir_all(plugin.0.join("build")).unwrap();
        fs::write(plugin.0.join("build/params.json"), json!({ "params": [{ "name": "Mix" }] }).to_string()).unwrap();

        let no_such_control = keep_chosen(&plugin.0, &json!({ "knobs": [{ "control": "Wobble" }] })).unwrap_err();
        let label_too_long = keep_chosen(&plugin.0, &json!({ "knobs": [{ "control": "Mix", "label": "Far too long a label" }] })).unwrap_err();

        assert!(no_such_control[0].contains("turns `Wobble`") && label_too_long[0].contains("its label"), "{no_such_control:?} {label_too_long:?}");
        assert!(!plugin.0.join(SIX_KNOBS_FILE).exists());
    }

    #[test]
    fn the_knobs_akais_starter_names_by_number_are_taken_out_of_a_plugins_own_manifest() {
        let plugin = TemporaryFolder::create("six-knobs-starter");
        let as_started = "{\n  \"name\": \"Dusty Tape\",\n  \"shortName\": \"Dusty Tape\",\n  \"knobs\": [{ \"parameter\": 0, \"label\": \"Gain\" }],\n  \"shiftKnobs\": [],\n  \"uid\": \"WFmo\"\n}";
        fs::write(plugin.0.join(MANIFEST), as_started).unwrap();

        take_out_of_the_manifest(&plugin.0).unwrap();
        let taken_out = fs::read_to_string(plugin.0.join(MANIFEST)).unwrap();
        take_out_of_the_manifest(&plugin.0).unwrap();

        assert_eq!(serde_json::from_str::<Value>(&taken_out).unwrap(), json!({ "name": "Dusty Tape", "uid": "WFmo" }));
        assert!(taken_out.find("name").unwrap() < taken_out.find("uid").unwrap(), "the rest stays in the order it was written");
        assert_eq!(fs::read_to_string(plugin.0.join(MANIFEST)).unwrap(), taken_out, "a manifest that names no knobs is left as it is");
    }

    #[test]
    fn an_effect_that_keeps_no_knobs_leaves_with_a_first_guess() {
        let plugin = TemporaryFolder::create("six-knobs-guessed");
        let built = plugin.0.join("build/Tests - VST - Stereo Delay");
        fs::create_dir_all(&built).unwrap();
        fs::write(built.join(MANIFEST), r#"{ "name": "Stereo Delay", "kind": "effect" }"#).unwrap();
        fs::write(plugin.0.join("build/params.json"), json!({ "params": controls_of_a_delay().iter().map(|name| json!({ "name": name })).collect::<Vec<_>>() }).to_string()).unwrap();

        write_into_the_built_plugin(&plugin.0).unwrap();

        let manifest: Value = serde_json::from_str(&fs::read_to_string(built.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!(manifest["knobs"], json!([{ "parameter": 1, "label": "Time" }, { "parameter": 2, "label": "Feedback" }, { "parameter": 3, "label": "Mix" }]));
        assert_eq!(manifest["shortName"], "Stereo Delay");
    }
}
