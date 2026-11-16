//! The shapes the screen and the engine exchange. Everything the screen shows is built here.

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use workbench_engine::agent::Event;
use workbench_engine::architect::{Answer, Idea, Question};
use workbench_engine::sdk::PluginKind;
use workbench_engine::ai_settings::AiSettings;
use workbench_engine::chat_model::ChosenModel;
use workbench_engine::codex::CodexChoices;
use workbench_engine::history::Version;
use workbench_engine::arranged_screen::{ArrangedItem, ArrangedLayer, ArrangedPage, ArrangedPicture};
use workbench_engine::control_parts::parts_as_json;
use workbench_engine::gate::GateReport;
use workbench_engine::library::{self, PluginSummary};
use workbench_engine::lucky_find::{Find, Hunt, Roll};
use workbench_engine::open_mpc::{self, Listed};
use workbench_engine::maker::Delivery;
use workbench_engine::pictures::{Motion, Picture, Placement};
use workbench_engine::plugin_tools::describe_step;
use workbench_engine::readiness::Need;
use workbench_engine::samples::Sample;
use workbench_engine::screen_layout::Area;

/// What the screen sends to start a new plugin.
#[derive(Debug, Deserialize)]
pub struct NewPlugin {
    pub name: String,
    pub maker: String,
    pub kind: String,
    pub description: String,
    /// The names the pictures attached to the description were kept under.
    #[serde(default)]
    pub pictures: Vec<String>,
}

/// What the screen sends to change a plugin: the musician's words and the pictures attached to them.
#[derive(Debug, Deserialize)]
pub struct ChangeAsked {
    pub words: String,
    #[serde(default)]
    pub pictures: Vec<String>,
}

/// What was picked on screen. The model and reasoning level may be left as "default".
/// Web lookups are allowed unless they were switched off.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPicks {
    /// The service the model belongs to. Left out, it is Anthropic.
    pub provider: Option<String>,
    pub model: Option<String>,
    /// Whether a model from another service can look at pictures, as that service's listing said.
    #[serde(default)]
    pub sees_pictures: bool,
    pub effort: Option<String>,
    pub web: Option<String>,
    /// How many tokens the AI's conversation may reach before older material is dropped. Left out, the AI
    /// service decides for itself.
    pub compact_at: Option<u64>,
}

/// What the screen sends the plugin architect: the idea so far, and what was asked and answered in earlier rounds.
#[derive(Debug, Deserialize)]
pub struct IdeaAsked {
    pub name: String,
    pub kind: String,
    pub description: String,
    #[serde(default)]
    pub answered: Vec<AnswerGiven>,
}

#[derive(Debug, Deserialize)]
pub struct AnswerGiven {
    pub question: String,
    pub answer: String,
}

impl IdeaAsked {
    pub fn as_idea(self) -> Result<Idea, String> {
        let kind = PluginKind::from_word(&self.kind).ok_or("A plugin is either an effect or an instrument.")?;
        let answered = self.answered.into_iter().map(|given| Answer { question: given.question, answer: given.answer }).collect();
        Ok(Idea { name: self.name, kind, description: self.description, answered })
    }
}

/// The roll of the dice the screen hands back when it sends the scout out, with the plugins already made.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HuntAsked {
    pub number: usize,
    pub kind: String,
    #[serde(default)]
    pub already_made: Vec<String>,
}

impl HuntAsked {
    pub fn as_hunt(self) -> Result<Hunt, String> {
        let kind = PluginKind::from_word(&self.kind).ok_or("A plugin is either an effect or an instrument.")?;
        let roll = Roll::numbered(self.number, kind).ok_or("The dice have no such number.")?;
        Ok(Hunt { roll, already_made: self.already_made })
    }
}

pub fn roll(roll: &Roll) -> Value {
    json!({ "number": roll.number(), "theme": roll.theme(), "kind": roll.kind.as_word() })
}

pub fn find(find: &Find) -> Value {
    json!({ "name": find.name, "story": find.story, "description": find.description, "source": find.source })
}

/// A plugin of the Open MPC catalog, with the name and the brief a remix of it starts from.
pub fn listed_for_remixing(listed: &Listed) -> Value {
    json!({
        "id": listed.id, "name": listed.name, "author": listed.author, "kind": listed.kind.as_word(), "summary": listed.summary,
        "style": listed.style, "license": listed.license, "source": listed.source, "screenshot": listed.screenshot,
        "remixName": listed.name_for_a_remix(), "brief": listed.brief_for_a_remix(), "aboutItsPicture": open_mpc::ABOUT_THE_SCREENSHOT,
    })
}

/// One of the architect's questions, for the screen to show with its choices as buttons.
pub fn question(question: &Question) -> Value {
    json!({ "asks": question.asks, "choices": question.choices, "pickSeveral": question.pick_several })
}

impl AiPicks {
    /// The picks as they apply to a Claude model.
    pub fn as_settings(&self) -> Result<AiSettings, String> {
        let settings = AiSettings::from_words(self.model.as_deref(), self.effort.as_deref()).map_err(|unknown| unknown.to_string())?;
        Ok(settings.with_web_access(self.web.as_deref() != Some(WEB_SWITCHED_OFF)))
    }

    /// The picks as they apply to a model reached through Codex. The reasoning level and web lookups carry over.
    pub fn as_codex_choices(&self) -> Result<CodexChoices, String> {
        let settings = AiSettings::from_words(None, self.effort.as_deref()).map_err(|unknown| unknown.to_string())?;
        Ok(CodexChoices { model: Some(self.as_chosen_model()?.id), effort: settings.effort, web_access: self.web.as_deref() != Some(WEB_SWITCHED_OFF) })
    }

    /// The picks as they apply to a model from any other service. Such a service charges for each web lookup,
    /// so those stay off unless they were switched on.
    pub fn as_chosen_model(&self) -> Result<ChosenModel, String> {
        let id = self.model.as_deref().map(str::trim).filter(|id| !id.is_empty()).ok_or(NO_MODEL_PICKED)?;
        let effort = AiSettings::from_words(None, self.effort.as_deref()).map_err(|unknown| unknown.to_string())?.effort;
        let looks_things_up = self.web.as_deref() == Some(WEB_SWITCHED_ON);
        Ok(ChosenModel { id: id.to_string(), sees_pictures: self.sees_pictures, effort, looks_things_up })
    }
}

const NO_MODEL_PICKED: &str = "Pick a model from the menu at the top first.";

/// Where the musician's MPC is, and whether its app is to be restarted once the plugin is there.
#[derive(Debug, Deserialize)]
pub struct MpcAsked {
    pub address: String,
    pub drive: Option<String>,
    #[serde(default)]
    pub restart: bool,
}

const WEB_SWITCHED_OFF: &str = "off";
const WEB_SWITCHED_ON: &str = "on";

/// Which AI the screen asked for.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AiChoice {
    ClaudeCode,
    Api,
}

pub fn need(need: &Need) -> Value {
    json!({ "name": need.name, "usedFor": need.used_for, "isThere": need.is_there, "isRequired": need.is_required, "howToGet": need.how_to_get, "canBeFetched": need.can_be_fetched })
}

pub fn sample(sample: &Sample) -> Value {
    json!({ "fileName": sample.file_name, "seconds": sample.seconds, "channels": sample.channels, "sampleRate": sample.sample_rate })
}

/// A picture on the plugin's screen. `version` changes whenever the picture is drawn again, so the
/// screen knows to fetch the new image. Rows and columns are counted from one, as in `pictures.json`.
/// `plays_every` is how many seconds one play takes when the plugin moves the picture by itself: the
/// screen then plays it round and round at that speed, whatever control the plugin uses to move it.
pub fn picture(picture: &Picture, version: u64, plays_every: Option<f64>) -> Value {
    let (motion, follows) = match (&picture.motion, plays_every) {
        (Motion::Still, _) => ("still", None),
        (Motion::Loops, _) | (Motion::Follows(_), Some(_)) => ("loops", None),
        (Motion::Follows(control), None) => ("follows", Some(control)),
    };
    let block = match &picture.placement {
        Placement::Backdrop => Value::Null,
        Placement::Slots(block) => json!({ "row": block.top_row + 1, "column": block.left_column + 1, "rows": block.rows, "columns": block.columns }),
    };
    json!({
        "name": picture.name, "page": picture.page, "slots": block, "frames": picture.frames,
        "motion": motion, "follows": follows, "loopSeconds": plays_every, "version": version
    })
}

/// The screen as it will be shown, page by page: where each control, heading and picture sits, in MPC pixels.
pub fn arrangement(pages: &[ArrangedPage]) -> Value {
    json!({ "pages": pages.iter().map(arranged_page).collect::<Vec<Value>>() })
}

fn arranged_page(page: &ArrangedPage) -> Value {
    json!({
        "name": page.name,
        "items": page.items.iter().map(arranged_item).collect::<Vec<Value>>(),
        "pictures": page.pictures.iter().map(arranged_picture).collect::<Vec<Value>>(),
        "artwork": page.artwork.iter().map(arranged_layer).collect::<Vec<Value>>(),
        "groupBoxes": page.group_boxes.iter().map(area).collect::<Vec<Value>>(),
    })
}

/// `slot` is the item's place in the page's list of controls in `ui.json`, counted from zero.
/// `parts` says how its inside was laid out by hand, and is null when the generator's own layout stands.
fn arranged_item(item: &ArrangedItem) -> Value {
    json!({
        "kind": item.kind.word(),
        "name": item.name,
        "slot": item.slot,
        "area": area(&item.area),
        "shownName": item.shown_name,
        "moved": item.has_moved(),
        "parts": item.parts.as_ref().map(parts_as_json),
    })
}

/// One layer of the artwork behind a page, with the pixels it covers. Words are measured in their own font.
fn arranged_layer(layer: &ArrangedLayer) -> Value {
    let frame = layer.frame;
    json!({ "id": layer.id, "area": { "x": frame.x, "y": frame.y, "width": frame.width, "height": frame.height }, "hidden": layer.hidden })
}

fn arranged_picture(picture: &ArrangedPicture) -> Value {
    json!({ "name": picture.name, "area": area(&picture.area) })
}

fn area(area: &Area) -> Value {
    json!({ "x": area.x, "y": area.y, "width": area.width, "height": area.height })
}

pub fn summary(plugin: &PluginSummary) -> Value {
    json!({ "name": plugin.name, "maker": plugin.maker, "kind": plugin.kind })
}

pub fn report(report: &GateReport) -> Value {
    let checks: Vec<Value> = report
        .checks
        .iter()
        .map(|check| json!({ "name": check.name, "outcome": check.outcome.label(), "details": check.outcome.details() }))
        .collect();
    json!({ "safe": report.may_be_sent(), "checks": checks })
}

/// One line of progress while the AI works. Returns nothing for events the screen does not show.
pub fn progress(event: &Event) -> Option<Value> {
    match event {
        Event::ModelSaid(text) => Some(json!({ "kind": "said", "text": text })),
        Event::ToolStarted { name, input } => Some(json!({ "kind": "step", "text": describe_step(name, input) })),
        Event::ToolFinished { succeeded: false, .. } => Some(json!({ "kind": "step-failed", "text": "that did not work; the AI has been told why" })),
        Event::ContextCompacted => Some(json!({ "kind": "step", "text": "making room: older material was dropped from the AI's context" })),
        Event::ToolFinished { .. } | Event::ContextUsed { .. } => None,
    }
}

/// How many tokens the AI's conversation holds, when the event says. The screen shows it beside the conversation.
pub fn context(event: &Event) -> Option<Value> {
    match event {
        Event::ContextUsed { tokens } => Some(json!({ "tokens": tokens })),
        _ => None,
    }
}

pub fn version(version: &Version) -> Value {
    json!({
        "number": version.number,
        "keptAt": version.kept_at,
        "keptBefore": version.kept_before,
        "isHowItIsNow": version.is_how_it_is_now,
        "bytes": version.size_in_bytes,
    })
}

pub fn finished(plugin_name: &str, delivery: &Delivery) -> Value {
    json!({
        "ok": true,
        "plugin": plugin_name,
        "closingWords": delivery.closing_words,
        "report": report(&delivery.report),
    })
}

pub fn failed(plugin_name: &str, explanation: &str) -> Value {
    json!({ "ok": false, "plugin": plugin_name, "error": explanation })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_from_another_service_is_named_exactly_as_that_service_listed_it() {
        let picks: AiPicks = serde_json::from_value(json!({ "provider": "openrouter", "model": " lab/builder-9 ", "seesPictures": true })).unwrap();

        assert_eq!(picks.as_chosen_model(), Ok(ChosenModel { id: "lab/builder-9".to_string(), sees_pictures: true, effort: None, looks_things_up: false }));
    }

    #[test]
    fn a_model_from_another_service_looks_things_up_only_when_that_was_switched_on() {
        let looks_things_up = |picks: Value| serde_json::from_value::<AiPicks>(picks).unwrap().as_chosen_model().unwrap().looks_things_up;

        assert!(looks_things_up(json!({ "provider": "openrouter", "model": "lab/builder-9", "web": "on" })));
        assert!(!looks_things_up(json!({ "provider": "openrouter", "model": "lab/builder-9", "web": "off" })));
        assert!(!looks_things_up(json!({ "provider": "openrouter", "model": "lab/builder-9" })));
    }

    #[test]
    fn the_effort_picked_goes_with_a_model_from_another_service() {
        let picks: AiPicks = serde_json::from_value(json!({ "provider": "openrouter", "model": "lab/thinker", "effort": "high" })).unwrap();
        let unpicked: AiPicks = serde_json::from_value(json!({ "provider": "openrouter", "model": "lab/thinker", "effort": "default" })).unwrap();

        assert_eq!(picks.as_chosen_model().unwrap().effort.map(|effort| effort.word()), Some("high"));
        assert_eq!(unpicked.as_chosen_model().unwrap().effort, None);
    }

    #[test]
    fn a_roll_goes_to_the_screen_and_comes_back_as_the_scouts_hunt() {
        let rolled = Roll::numbered(4, PluginKind::Instrument).unwrap();
        let handed_back: HuntAsked = serde_json::from_value(json!({ "number": 4, "kind": "instrument", "alreadyMade": ["Dusty Tape"] })).unwrap();
        let no_such_face: HuntAsked = serde_json::from_value(json!({ "number": 11, "kind": "effect" })).unwrap();

        assert_eq!(roll(&rolled), json!({ "number": 4, "theme": "Space", "kind": "instrument" }));
        let hunt = handed_back.as_hunt().unwrap();
        assert_eq!((hunt.roll, hunt.already_made), (rolled, vec!["Dusty Tape".to_string()]));
        assert!(no_such_face.as_hunt().is_err());
    }

    #[test]
    fn another_service_cannot_be_asked_without_a_model_being_picked() {
        let picks: AiPicks = serde_json::from_value(json!({ "provider": "openrouter", "model": "" })).unwrap();

        assert_eq!(picks.as_chosen_model(), Err(NO_MODEL_PICKED.to_string()));
    }

    #[test]
    fn where_the_mpc_is_arrives_without_a_restart_unless_one_was_asked_for() {
        let asked: MpcAsked = serde_json::from_value(json!({ "address": "192.168.1.50", "drive": null })).unwrap();

        assert_eq!((asked.address.as_str(), asked.drive, asked.restart), ("192.168.1.50", None, false));
    }

    #[test]
    fn a_tool_request_becomes_a_plain_language_step() {
        let input = json!({ "path": "src/Echo.cpp" });

        let line = progress(&Event::ToolStarted { name: "write_plugin_file", input: &input }).unwrap();

        assert_eq!(line, json!({ "kind": "step", "text": "writing src/Echo.cpp" }));
    }

    #[test]
    fn a_tool_that_worked_adds_no_extra_line() {
        assert!(progress(&Event::ToolFinished { name: "check_plugin", succeeded: true }).is_none());
        assert!(progress(&Event::ToolFinished { name: "check_plugin", succeeded: false }).is_some());
        assert!(progress(&Event::ContextUsed { tokens: 9 }).is_none());
        assert!(progress(&Event::ContextCompacted).unwrap()["text"].as_str().unwrap().starts_with("making room"));
        assert_eq!(context(&Event::ContextUsed { tokens: 41_200 }).unwrap()["tokens"], 41_200);
        assert!(context(&Event::ContextCompacted).is_none());
    }

    #[test]
    fn the_ai_choice_is_read_from_the_words_the_screen_uses() {
        assert_eq!(serde_json::from_str::<AiChoice>("\"claude-code\"").unwrap(), AiChoice::ClaudeCode);
        assert_eq!(serde_json::from_str::<AiChoice>("\"api\"").unwrap(), AiChoice::Api);
    }

    #[test]
    fn picks_from_the_screen_become_settings_and_default_means_unset() {
        let picks: AiPicks = serde_json::from_str(r#"{"model": "haiku", "effort": "default"}"#).unwrap();

        let settings = picks.as_settings().unwrap();

        assert_eq!(settings.model.unwrap().id(), "claude-haiku-5-5");
        assert!(settings.effort.is_none());
    }

    #[test]
    fn picks_for_the_chatgpt_sign_in_carry_the_model_the_reasoning_level_and_web_lookups() {
        let picks: AiPicks = serde_json::from_str(r#"{"provider": "codex", "model": "gpt-6.1-sol", "effort": "high", "web": "off"}"#).unwrap();
        let no_model: AiPicks = serde_json::from_str(r#"{"provider": "codex"}"#).unwrap();

        let choices = picks.as_codex_choices().unwrap();

        assert_eq!(choices.model.as_deref(), Some("gpt-6.1-sol"));
        assert_eq!(choices.effort.unwrap().word(), "high");
        assert!(!choices.web_access);
        assert!(no_model.as_codex_choices().is_err());
    }

    #[test]
    fn web_lookups_are_allowed_unless_switched_off() {
        let untouched: AiPicks = serde_json::from_str("{}").unwrap();
        let switched_off: AiPicks = serde_json::from_str(r#"{"web": "off"}"#).unwrap();

        assert!(untouched.as_settings().unwrap().web_access);
        assert!(!switched_off.as_settings().unwrap().web_access);
    }

    #[test]
    fn a_picture_the_plugin_moves_by_itself_plays_round_and_round_at_the_plugins_speed() {
        let written = r#"{ "pictures": [{ "name": "reels", "page": "MAIN", "where": { "row": 3, "column": 1 }, "motion": { "follows": "Motion" }, "frames": 16 }] }"#;
        let pictures = workbench_engine::pictures::pictures_written_in(written).unwrap();

        let reels = picture(&pictures[0], 7, Some(1.6));

        assert_eq!((reels["motion"].as_str(), reels["follows"].as_str(), reels["loopSeconds"].as_f64()), (Some("loops"), None, Some(1.6)));
    }

    #[test]
    fn a_picture_is_described_by_its_slots_frames_and_motion() {
        let written = r#"{ "pictures": [
            { "name": "needle", "page": "MAIN", "where": { "row": 3, "column": 2, "columns": 2 }, "motion": { "follows": "Drive" }, "frames": 8 },
            { "name": "paper", "page": "MAIN", "where": "backdrop" }
        ]}"#;
        let pictures = workbench_engine::pictures::pictures_written_in(written).unwrap();

        let needle = picture(&pictures[0], 7, None);
        let paper = picture(&pictures[1], 7, None);

        assert_eq!(needle["slots"], json!({ "row": 3, "column": 2, "rows": 1, "columns": 2 }));
        assert_eq!((needle["motion"].as_str(), needle["follows"].as_str(), needle["frames"].as_u64()), (Some("follows"), Some("Drive"), Some(8)));
        assert_eq!((paper["slots"].is_null(), paper["motion"].as_str(), paper["version"].as_u64()), (true, Some("still"), Some(7)));
    }

    #[test]
    fn an_arrangement_says_where_each_item_sits_and_whether_it_was_moved() {
        let layout = workbench_engine::screen_layout::ScreenLayout::from_json(&json!({ "pages": [{ "name": "MAIN", "controls": ["Time", "Mix"] }] }));
        let look = workbench_engine::look::look_written_in(
            r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "control", "name": "Mix", "place": { "x": 600, "y": 300 }, "shownName": "BLEND" }] }] }"#,
        );
        let pages = workbench_engine::arranged_screen::arranged(&layout, &[], &look.unwrap());

        let shown = arrangement(&pages);

        let items = &shown["pages"][0]["items"];
        assert_eq!(items[0], json!({ "kind": "control", "name": "Time", "slot": 0, "area": { "x": 23, "y": 45, "width": 278, "height": 107 }, "shownName": null, "moved": false, "parts": null }));
        assert_eq!(items[1], json!({ "kind": "control", "name": "Mix", "slot": 1, "area": { "x": 600, "y": 300, "width": 278, "height": 107 }, "shownName": "BLEND", "moved": true, "parts": null }));
        assert_eq!(shown["pages"][0]["groupBoxes"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn an_arrangement_lists_a_pages_artwork_and_how_a_control_was_laid_out_by_hand() {
        let layout = workbench_engine::screen_layout::ScreenLayout::from_json(&json!({ "pages": [{ "name": "MAIN", "controls": ["Time"] }] }));
        let look = workbench_engine::look::look_written_in(
            r##"{ "pages": [{ "page": "MAIN",
                "artwork": [{ "kind": "block", "id": 4, "area": { "x": -20, "y": 0, "width": 300, "height": 100 }, "colour": "#ff0000", "hidden": true }],
                "items": [{ "kind": "control", "name": "Time", "parts": { "art": { "x": 0, "y": 0, "width": 150, "height": 150 } } }] }] }"##,
        );

        let shown = arrangement(&workbench_engine::arranged_screen::arranged(&layout, &[], &look.unwrap()));

        let page = &shown["pages"][0];
        assert_eq!(page["artwork"], json!([{ "id": 4, "area": { "x": -20, "y": 0, "width": 300, "height": 100 }, "hidden": true }]));
        assert_eq!(page["items"][0]["parts"], json!({ "art": { "x": 0, "y": 0, "width": 150, "height": 150 } }));
        assert_eq!(page["items"][0]["area"], json!({ "x": 23, "y": 45, "width": 150, "height": 150 }));
    }

    #[test]
    fn an_empty_report_is_never_shown_as_safe() {
        assert_eq!(report(&GateReport::default()), json!({ "safe": false, "checks": [] }));
    }
}

/// A folder for the plugins, as the settings show it: where it is, whether it is there, and how many plugins it holds.
pub fn plugins_folder(folder: &Path) -> Value {
    json!({ "path": folder.display().to_string(), "found": folder.is_dir(), "plugins": library::list_plugins(folder).len() })
}
