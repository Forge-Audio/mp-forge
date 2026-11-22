//! The architect: before anything is built, it asks the musician a few questions about their idea and offers
//! choices for each, so what gets built is more their own. For a plugin it is the plugin architect. For the
//! MPC Sample, where what is made takes over the whole device, it is the OS architect and asks about that.
//!
//! The architect can only ask. Reached through a model the workbench talks to, its one tool is the form it
//! writes its questions on. Reached through Claude Code, it has no tools at all and writes the same form as its
//! answer. Either way it never sees a plugin's files and nothing it writes is ever run.

use std::fmt;

use serde_json::{Value, json};

use crate::agent::{AgentError, Brief, Entry, Model, ModelError, ToolDefinition, ToolInput};
use crate::ai_settings::AiSettings;
use crate::claude_code;
use crate::sdk::PluginKind;
use crate::shown_words::{form_written_in, one_line, shortened};
use crate::stop_signal::StopSignal;

const HOW_TO_BE_THE_ARCHITECT: &str = concat!(include_str!("../../directives/plugin-architect.md"), "\n", include_str!("../../directives/what-mp-forge-can-build.md"));
const HOW_TO_BE_THE_OS_ARCHITECT: &str = include_str!("../../directives/sample-architect.md");
const FORM: &str = "offer_questions";
const ABOUT_THE_FORM: &str = "Hands your questions to the musician. This is the only thing you can do, so always use it.";
/// Claude Code is given no form to fill in, so it is told to write one out.
const WRITE_THE_FORM_OUT: &str = "\n\nYou have no tools. Answer with the form itself and nothing else, as one JSON object: \
                                  {\"questions\": [{\"asks\": \"...\", \"choices\": [\"...\", \"...\", \"...\"], \"pickSeveral\": false}]}";
const MOST_QUESTIONS: usize = 5;
const FEWEST_CHOICES: usize = 2;
const MOST_CHOICES: usize = 5;
const LONGEST_QUESTION: usize = 240;
const LONGEST_CHOICE: usize = 90;
const LONGEST_ANSWER: usize = 400;

/// What the musician has said of the plugin so far.
pub struct Idea {
    pub name: String,
    pub kind: PluginKind,
    pub description: String,
    /// The questions of earlier rounds, each with what the musician answered.
    pub answered: Vec<Answer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub question: String,
    pub answer: String,
}

/// One question for the musician, with the choices they can pick from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub asks: String,
    pub choices: Vec<String>,
    /// Whether more than one of the choices can be picked together.
    pub pick_several: bool,
}

#[derive(Debug)]
pub enum ArchitectError {
    Ai(AgentError),
    /// The AI answered, but not with questions the musician could be shown.
    NothingToAsk,
}

impl fmt::Display for ArchitectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchitectError::Ai(error) => error.fmt(formatter),
            ArchitectError::NothingToAsk => write!(formatter, "The architect did not come back with any questions. Ask it again, or build without it."),
        }
    }
}

impl From<AgentError> for ArchitectError {
    fn from(problem: AgentError) -> Self {
        ArchitectError::Ai(problem)
    }
}

impl From<ModelError> for ArchitectError {
    fn from(problem: ModelError) -> Self {
        ArchitectError::Ai(AgentError::Model(problem))
    }
}

/// Asks a model the workbench talks to directly. The model is handed the form and nothing else.
pub fn ask_model(model: &mut dyn Model, idea: &Idea) -> Result<Vec<Question>, ArchitectError> {
    let form = [ToolDefinition { name: FORM, description: ABOUT_THE_FORM, input_schema: shape_of_the_form() }];
    let said = [Entry::User(what_the_musician_said(idea))];
    let turn = model.next_turn(&Brief { instructions: how_to_be_the_architect_of(idea), tools: &form, history: &said })?;
    let filled_in = turn.tool_calls.iter().find(|call| call.name == FORM).and_then(|call| match &call.input {
        ToolInput::Valid(form) => Some(questions_on(form)),
        ToolInput::Unparseable(_) => None,
    });
    worth_asking(filled_in.unwrap_or_else(|| questions_written_in(&turn.text)))
}

/// Asks through the musician's own Claude Code, which is given no tools at all for this.
pub fn ask_claude_code(idea: &Idea, settings: AiSettings, stop: &StopSignal) -> Result<Vec<Question>, ArchitectError> {
    let instructions = format!("{}{WRITE_THE_FORM_OUT}", how_to_be_the_architect_of(idea));
    let answer = claude_code::answer_once(&instructions, &what_the_musician_said(idea), settings, stop)?;
    worth_asking(questions_written_in(&answer))
}

fn how_to_be_the_architect_of(idea: &Idea) -> &'static str {
    if idea.kind == PluginKind::SampleApp { HOW_TO_BE_THE_OS_ARCHITECT } else { HOW_TO_BE_THE_ARCHITECT }
}

fn what_is_being_made(idea: &Idea) -> String {
    if idea.kind == PluginKind::SampleApp {
        return format!("It is called \"{}\". It is for the MPC Sample.", idea.name);
    }
    format!("The plugin is called \"{}\". It is an {}.", idea.name, idea.kind.as_word())
}

/// The idea with what was asked and answered in earlier rounds, worded for the architect.
fn what_the_musician_said(idea: &Idea) -> String {
    let opening = format!("{}\n\nThe musician's idea:\n\n{}", what_is_being_made(idea), idea.description.trim());
    if idea.answered.is_empty() {
        return opening;
    }
    let answered: Vec<String> = idea.answered.iter().map(|answer| format!("- {} {}", answer.question.trim(), shortened(&answer.answer, LONGEST_ANSWER))).collect();
    format!("{opening}\n\nWhat you already asked, and what they answered:\n\n{}\n\nAsk about other things now, building on these answers.", answered.join("\n"))
}

fn shape_of_the_form() -> Value {
    let question = json!({
        "type": "object",
        "properties": {
            "asks": { "type": "string", "description": "The question, one short sentence, spoken to the musician as \"you\"." },
            "choices": { "type": "array", "items": { "type": "string" }, "description": "Three or four short, vivid choices." },
            "pickSeveral": { "type": "boolean", "description": "True only when the choices can sensibly be combined." },
        },
        "required": ["asks", "choices"],
    });
    json!({ "type": "object", "properties": { "questions": { "type": "array", "items": question } }, "required": ["questions"] })
}

/// The questions on a form, with whatever could not be shown to the musician left out.
fn questions_on(form: &Value) -> Vec<Question> {
    let written = form["questions"].as_array().map(Vec::as_slice).unwrap_or_default();
    written.iter().filter_map(question_from).take(MOST_QUESTIONS).collect()
}

/// The questions in an answer that is the form written out, perhaps with other words round it.
fn questions_written_in(answer: &str) -> Vec<Question> {
    form_written_in(answer).map(|form| questions_on(&form)).unwrap_or_default()
}

fn question_from(written: &Value) -> Option<Question> {
    let asks = one_line(written["asks"].as_str()?, LONGEST_QUESTION)?;
    let choices: Vec<String> = written["choices"].as_array()?.iter().filter_map(Value::as_str).filter_map(|choice| one_line(choice, LONGEST_CHOICE)).take(MOST_CHOICES).collect();
    let enough_to_pick_from = choices.len() >= FEWEST_CHOICES;
    enough_to_pick_from.then(|| Question { asks, choices, pick_several: written["pickSeveral"].as_bool().unwrap_or(false) })
}

fn worth_asking(questions: Vec<Question>) -> Result<Vec<Question>, ArchitectError> {
    if questions.is_empty() { Err(ArchitectError::NothingToAsk) } else { Ok(questions) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idea_for_the_mpc_sample_goes_to_the_os_architect_who_asks_about_the_whole_device() {
        let idea = Idea { name: "Night Bus".to_string(), kind: PluginKind::SampleApp, description: "dusty chords".to_string(), answered: Vec::new() };

        assert!(how_to_be_the_architect_of(&idea).starts_with("# The OS architect"));
        assert!(how_to_be_the_architect_of(&idea).contains("what the pads do and how they are laid out"));
        assert!(!how_to_be_the_architect_of(&idea).contains("touch screen of pages"));
        assert!(what_the_musician_said(&idea).starts_with("It is called \"Night Bus\". It is for the MPC Sample.\n\nThe musician's idea:\n\ndusty chords"));
    }
    use crate::agent::test_support::{ScriptedModel, calls, says};

    fn idea() -> Idea {
        Idea { name: "Dusty Tape".to_string(), kind: PluginKind::Effect, description: " a worn tape echo ".to_string(), answered: Vec::new() }
    }

    fn two_questions() -> Value {
        json!({ "questions": [
            { "asks": "How worn should the tape feel?", "choices": ["Fresh reel", "Left in a hot car", "Chewed"], "pickSeveral": false },
            { "asks": "Which extras do you want?", "choices": ["Freeze", "Reverse"], "pickSeveral": true },
        ] })
    }

    #[test]
    fn a_model_is_handed_only_the_form_and_its_questions_are_read_from_it() {
        let mut model = ScriptedModel::playing([calls(FORM, two_questions())]);

        let questions = ask_model(&mut model, &idea()).unwrap();

        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0], Question { asks: "How worn should the tape feel?".to_string(), choices: vec!["Fresh reel".to_string(), "Left in a hot car".to_string(), "Chewed".to_string()], pick_several: false });
        assert!(questions[1].pick_several);
        assert_eq!(model.histories_seen[0], [Entry::User("The plugin is called \"Dusty Tape\". It is an effect.\n\nThe musician's idea:\n\na worn tape echo".to_string())]);
    }

    #[test]
    fn a_form_written_out_in_words_is_read_just_the_same() {
        let mut model = ScriptedModel::playing([says(&format!("Here you go:\n```json\n{}\n```", two_questions()))]);

        assert_eq!(ask_model(&mut model, &idea()).unwrap().len(), 2);
    }

    #[test]
    fn an_answer_with_no_questions_in_it_is_said_plainly() {
        let mut model = ScriptedModel::playing([says("A tape echo sounds great, shall I build it?")]);

        let refusal = ask_model(&mut model, &idea()).unwrap_err();

        assert!(refusal.to_string().contains("did not come back with any questions"));
    }

    #[test]
    fn what_cannot_be_shown_is_left_out_and_long_words_are_cut_short() {
        let form = json!({ "questions": [
            { "asks": "Only one choice?", "choices": ["Yes"] },
            { "asks": "", "choices": ["A", "B"] },
            { "asks": "Spread\nover lines?", "choices": ["  A  ", "", 7, "B".repeat(LONGEST_CHOICE + 20)] },
            { "choices": ["A", "B"] },
        ] });

        let questions = questions_on(&form);

        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].asks, "Spread over lines?");
        assert_eq!(questions[0].choices[0], "A");
        assert_eq!(questions[0].choices[1].chars().count(), LONGEST_CHOICE + 1);
    }

    #[test]
    fn no_more_questions_or_choices_are_shown_than_fit() {
        let question = json!({ "asks": "Which?", "choices": ["1", "2", "3", "4", "5", "6", "7"] });
        let form = json!({ "questions": vec![question; MOST_QUESTIONS + 3] });

        let questions = questions_on(&form);

        assert_eq!(questions.len(), MOST_QUESTIONS);
        assert_eq!(questions[0].choices.len(), MOST_CHOICES);
    }

    #[test]
    fn a_later_round_is_told_what_was_already_asked_and_answered() {
        let answered = vec![Answer { question: "How worn should the tape feel?".to_string(), answer: "Left in a hot car".to_string() }];

        let said = what_the_musician_said(&Idea { answered, ..idea() });

        assert!(said.contains("- How worn should the tape feel? Left in a hot car"));
        assert!(said.ends_with("Ask about other things now, building on these answers."));
    }

    #[test]
    fn the_architect_is_told_it_only_asks_and_what_can_really_be_built() {
        assert!(HOW_TO_BE_THE_ARCHITECT.contains("You only ask."));
        assert!(HOW_TO_BE_THE_ARCHITECT.contains("An effect changes sound that is played through it. It hears no notes."));
    }
}
