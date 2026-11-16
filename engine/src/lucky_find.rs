//! "I'm feeling lucky": dice pick a theme, a scout looks for something new and interesting in it, and what it
//! finds becomes the idea for a plugin.
//!
//! The scout can only look things up and answer. Reached through a model the workbench talks to, its one tool
//! is the form it writes its find on, beside whatever web lookups the musician has switched on for that model.
//! Reached through Claude Code, it keeps that program's two read-only web tools when lookups are on and writes
//! the same form as its answer. Either way it never sees a plugin's files and nothing it writes is ever run:
//! its find is only words, handed to the builder like any idea the musician typed.

use std::fmt;

use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};

use crate::agent::{AgentError, Brief, Entry, Model, ModelError, ModelTurn, StopReason, ToolDefinition, ToolInput};
use crate::ai_settings::AiSettings;
use crate::claude_code;
use crate::maker::name_made_usable;
use crate::sdk::PluginKind;
use crate::shown_words::{form_written_in, one_line, secure_web_address, shortened};
use crate::stop_signal::StopSignal;

/// What each face of the dice stands for, from one to ten.
pub const THEMES: [&str; 10] = ["Art", "Science", "Nature", "Space", "History", "Machines", "Film and games", "Places", "Mathematics", "Myths and legends"];
const KINDS: [PluginKind; 2] = [PluginKind::Effect, PluginKind::Instrument];

const HOW_TO_SCOUT: &str = concat!(include_str!("../../directives/lucky-scout.md"), "\n", include_str!("../../directives/what-mp-forge-can-build.md"));
const FORM: &str = "report_find";
const ABOUT_THE_FORM: &str = "Hands your find, and the plugin idea made from it, to the musician. Use it once, when you have finished looking.";
/// Claude Code is given no form to fill in, so it is told to write one out.
const WRITE_THE_FORM_OUT: &str = "\n\nYou have no form to fill in. When you have finished looking, answer with the form itself and nothing else, \
                                  as one JSON object: {\"name\": \"...\", \"story\": \"...\", \"description\": \"...\", \"source\": \"https://...\"}";
/// A long run of lookups can pause and be picked up again. This is how often that is allowed before giving up.
const MOST_TURNS: usize = 4;
const LONGEST_NAME: usize = 24;
const LONGEST_STORY: usize = 600;
const LONGEST_DESCRIPTION: usize = 2400;
const NO_RANDOMNESS: &str = "This computer could not roll the dice.";

/// One throw of the dice: a number from one to ten, which names the theme, and the kind of plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roll {
    number: usize,
    pub kind: PluginKind,
}

impl Roll {
    /// A roll as the screen hands it back. A number the dice do not have is no roll.
    pub fn numbered(number: usize, kind: PluginKind) -> Option<Self> {
        (1..=THEMES.len()).contains(&number).then_some(Self { number, kind })
    }

    pub fn number(&self) -> usize {
        self.number
    }

    pub fn theme(&self) -> &'static str {
        THEMES[self.number - 1]
    }
}

/// Rolls the dice, with this computer's own source of chance.
pub fn rolled() -> Result<Roll, String> {
    let chance = SystemRandom::new();
    let mut failed = false;
    let roll = roll_from(|| {
        let mut byte = [0u8; 1];
        failed |= chance.fill(&mut byte).is_err();
        byte[0]
    });
    if failed { Err(NO_RANDOMNESS.to_string()) } else { Ok(roll) }
}

fn roll_from(mut next_byte: impl FnMut() -> u8) -> Roll {
    let number = fair_pick(THEMES.len(), &mut next_byte) + 1;
    let kind = KINDS[fair_pick(KINDS.len(), &mut next_byte)];
    Roll { number, kind }
}

/// A place from nought up to, not counting, `sides`, each as likely as the next. A byte from the uneven tail
/// of its range is thrown again, so no face is favoured. Chance that never lands is given up on.
fn fair_pick(sides: usize, next_byte: &mut impl FnMut() -> u8) -> usize {
    const THROWS_BEFORE_GIVING_UP: usize = 64;
    let even_range = (usize::from(u8::MAX) + 1) / sides * sides;
    let landed = (0..THROWS_BEFORE_GIVING_UP).map(|_| usize::from(next_byte())).find(|thrown| *thrown < even_range);
    landed.unwrap_or(0) % sides
}

/// What the scout is sent out with.
pub struct Hunt {
    pub roll: Roll,
    /// The names of the plugins the musician already has, so the find leads somewhere else.
    pub already_made: Vec<String>,
}

/// What the scout came back with: a plugin idea, and the find it was made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Find {
    pub name: String,
    /// What was found and why it is worth hearing, for the musician.
    pub story: String,
    /// The brief for the builder.
    pub description: String,
    /// The web page the scout leaned on most, when it searched.
    pub source: Option<String>,
}

#[derive(Debug)]
pub enum LuckyError {
    Ai(AgentError),
    /// The AI answered, but not with a find a plugin could be built from.
    NothingFound,
}

impl fmt::Display for LuckyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LuckyError::Ai(error) => error.fmt(formatter),
            LuckyError::NothingFound => write!(formatter, "The scout came back empty-handed. Roll again."),
        }
    }
}

impl From<AgentError> for LuckyError {
    fn from(problem: AgentError) -> Self {
        LuckyError::Ai(problem)
    }
}

impl From<ModelError> for LuckyError {
    fn from(problem: ModelError) -> Self {
        LuckyError::Ai(AgentError::Model(problem))
    }
}

/// Asks a model the workbench talks to directly. The model is handed the form, and can look things up only
/// where the musician switched that on for it.
pub fn ask_model(model: &mut dyn Model, hunt: &Hunt) -> Result<Find, LuckyError> {
    let form = [ToolDefinition { name: FORM, description: ABOUT_THE_FORM, input_schema: shape_of_the_form() }];
    let mut said = vec![Entry::User(what_the_dice_said(hunt))];
    for _ in 0..MOST_TURNS {
        let turn = model.next_turn(&Brief { instructions: HOW_TO_SCOUT, tools: &form, history: &said })?;
        if turn.stop != StopReason::Paused {
            return find_in(&turn).ok_or(LuckyError::NothingFound);
        }
        said.push(Entry::Model(turn));
    }
    Err(LuckyError::NothingFound)
}

/// Asks through the musician's own Claude Code, which keeps only its two web tools, and those only when the
/// settings allow web lookups.
pub fn ask_claude_code(hunt: &Hunt, settings: AiSettings, stop: &StopSignal) -> Result<Find, LuckyError> {
    let instructions = format!("{HOW_TO_SCOUT}{WRITE_THE_FORM_OUT}");
    let answer = claude_code::answer_once_looking_things_up(&instructions, &what_the_dice_said(hunt), settings, stop)?;
    form_written_in(&answer).and_then(|form| find_on(&form)).ok_or(LuckyError::NothingFound)
}

fn what_the_dice_said(hunt: &Hunt) -> String {
    let roll = hunt.roll;
    let opening = format!("The dice rolled {}: {}. The plugin is to be an {}.", roll.number(), roll.theme(), roll.kind.as_word());
    if hunt.already_made.is_empty() {
        return opening;
    }
    format!("{opening}\n\nThe plugins the musician already has: {}.", hunt.already_made.join(", "))
}

fn shape_of_the_form() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "The plugin's name: two or three words, at most 24 characters, letters, digits and spaces." },
            "story": { "type": "string", "description": "Two or three plain sentences for the musician: what you found and why it is worth hearing." },
            "description": { "type": "string", "description": "The brief for the builder: what the plugin does, its controls and its look." },
            "source": { "type": "string", "description": "The address of the web page you leaned on most, when you searched." },
        },
        "required": ["name", "story", "description"],
    })
}

/// The find in a turn: on the form when the model filled it in, otherwise written out in its words.
fn find_in(turn: &ModelTurn) -> Option<Find> {
    let filled_in = turn.tool_calls.iter().find(|call| call.name == FORM).and_then(|call| match &call.input {
        ToolInput::Valid(form) => Some(form.clone()),
        ToolInput::Unparseable(_) => None,
    });
    filled_in.or_else(|| form_written_in(&turn.text)).and_then(|form| find_on(&form))
}

/// The find on a form, or nothing when a part a plugin cannot be built without is missing.
fn find_on(form: &Value) -> Option<Find> {
    let description = form["description"].as_str().map(|brief| shortened(brief, LONGEST_DESCRIPTION)).filter(|brief| !brief.is_empty())?;
    Some(Find {
        name: name_made_usable(form["name"].as_str()?, LONGEST_NAME)?,
        story: one_line(form["story"].as_str()?, LONGEST_STORY)?,
        description,
        source: form["source"].as_str().and_then(secure_web_address),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::test_support::{ScriptedModel, calls, says};

    fn hunt() -> Hunt {
        Hunt { roll: Roll::numbered(4, PluginKind::Instrument).unwrap(), already_made: vec!["Dusty Tape".to_string(), "Glass Pad".to_string()] }
    }

    fn form() -> Value {
        json!({ "name": "Pulsar Gate", "story": "A dead star\n ticks like a clock.", "description": "A gate that opens in a pulsar's rhythm.", "source": "https://example.org/pulsar" })
    }

    fn bytes(thrown: &[u8]) -> impl FnMut() -> u8 + '_ {
        let mut waiting = thrown.iter().copied();
        move || waiting.next().unwrap_or(0)
    }

    #[test]
    fn every_face_of_the_dice_has_a_theme_and_no_other_number_is_a_roll() {
        let themes: Vec<&str> = (1..=10).map(|number| Roll::numbered(number, PluginKind::Effect).unwrap().theme()).collect();

        assert_eq!(themes, THEMES);
        assert!(Roll::numbered(0, PluginKind::Effect).is_none());
        assert!(Roll::numbered(11, PluginKind::Effect).is_none());
    }

    #[test]
    fn a_roll_is_a_number_from_one_to_ten_and_a_kind() {
        assert_eq!(roll_from(bytes(&[0, 0])), Roll { number: 1, kind: PluginKind::Effect });
        assert_eq!(roll_from(bytes(&[19, 1])), Roll { number: 10, kind: PluginKind::Instrument });
        assert_eq!(roll_from(bytes(&[249, 255])), Roll { number: 10, kind: PluginKind::Instrument });
    }

    #[test]
    fn a_byte_from_the_uneven_tail_is_thrown_again() {
        assert_eq!(roll_from(bytes(&[250, 255, 13, 0])).number(), 4, "250 and up would favour the first six faces");
    }

    #[test]
    fn every_face_comes_up_equally_often_over_all_the_bytes_that_count() {
        let mut times_rolled = [0usize; 10];
        for byte in 0..250u8 {
            times_rolled[roll_from(bytes(&[byte, 0])).number() - 1] += 1;
        }

        assert_eq!(times_rolled, [25; 10]);
    }

    #[test]
    fn the_real_dice_roll_within_their_faces() {
        let roll = rolled().unwrap();

        assert!((1..=10).contains(&roll.number()));
    }

    #[test]
    fn the_scout_is_told_the_roll_and_what_the_musician_already_has() {
        let mut model = ScriptedModel::playing([calls(FORM, form())]);

        ask_model(&mut model, &hunt()).unwrap();

        let Entry::User(said) = &model.histories_seen[0][0] else { panic!("the scout was told nothing") };
        assert!(said.contains("The dice rolled 4: Space. The plugin is to be an instrument."));
        assert!(said.contains("Dusty Tape, Glass Pad"));
    }

    #[test]
    fn a_find_on_the_form_is_made_fit_to_show() {
        let mut model = ScriptedModel::playing([calls(FORM, form())]);

        let find = ask_model(&mut model, &hunt()).unwrap();

        assert_eq!(find, Find {
            name: "Pulsar Gate".to_string(),
            story: "A dead star ticks like a clock.".to_string(),
            description: "A gate that opens in a pulsar's rhythm.".to_string(),
            source: Some("https://example.org/pulsar".to_string()),
        });
    }

    #[test]
    fn a_find_written_out_in_words_is_read_too() {
        let mut model = ScriptedModel::playing([says(&format!("Here is my find: {}", form()))]);

        assert_eq!(ask_model(&mut model, &hunt()).unwrap().name, "Pulsar Gate");
    }

    #[test]
    fn a_paused_run_of_lookups_is_picked_up_again() {
        let paused = ModelTurn { stop: StopReason::Paused, ..says("") };
        let mut model = ScriptedModel::playing([paused, calls(FORM, form())]);

        assert_eq!(ask_model(&mut model, &hunt()).unwrap().name, "Pulsar Gate");
        assert_eq!(model.histories_seen[1].len(), 2, "the paused turn is handed back so the model carries on from it");
    }

    #[test]
    fn an_answer_without_a_find_is_an_error() {
        let mut nothing = ScriptedModel::playing([says("I could not think of anything.")]);
        let mut no_brief = ScriptedModel::playing([calls(FORM, json!({ "name": "Pulsar Gate", "story": "A star." }))]);

        assert!(matches!(ask_model(&mut nothing, &hunt()), Err(LuckyError::NothingFound)));
        assert!(matches!(ask_model(&mut no_brief, &hunt()), Err(LuckyError::NothingFound)));
    }
}
