//! What the screen can ask the app to do.
//!
//! Quick questions are answered directly. Making or changing a plugin takes minutes, so those start
//! in the background and report back through events: `work-progress` lines, then one `work-finished`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};
use tauri::ipc::{Request, Response};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use workbench_engine::agent::Event;
use workbench_engine::architect::{self, Question};
use workbench_engine::arranged_screen::arranged_screen_of;
use workbench_engine::art_shelf::ArtShelf;
use workbench_engine::artwork;
use workbench_engine::attachments;
use workbench_engine::chat_model::{ChatModel, ChosenModel};
use workbench_engine::plan_model::PlanModel;
use workbench_engine::claude::Claude;
use workbench_engine::codex::{self, CodexChoices, SignIn};
use workbench_engine::app_sending::Doors;
use workbench_engine::delivery::Mpc;
use workbench_engine::fonts;
use workbench_engine::guide::{self, Facts, GuideError, Words};
use workbench_engine::history;
use workbench_engine::layers::layer_from;
use workbench_engine::library;
use workbench_engine::link_program;
use workbench_engine::look::{look_from, look_of};
use workbench_engine::lucky_find::{self, Find, Hunt};
use workbench_engine::ollama;
use workbench_engine::online;
use workbench_engine::open_mpc;
use workbench_engine::open_mpc_listing::NotListed;
use workbench_engine::origin::Origin;
use workbench_engine::page_art;
use workbench_engine::picture_clock;
use workbench_engine::pictures::Picture;
use workbench_engine::process::command_for;
use workbench_engine::project_file::{NotTakenIn, PROJECT_FILE_ENDING};
use workbench_engine::readiness;
use workbench_engine::maker::{Connection, Delivery, Listed, MakeError, Maker, PluginRequest, PutOnACard, Sending, SentToTheSample};
use workbench_engine::providers::{ANTHROPIC, Provider};
use workbench_engine::samples::SampleShelf;
use workbench_engine::sdk::{PluginIdentity, PluginKind, Sdk, built_screen_file, built_skin_image};
use workbench_engine::stop_signal::StopSignal;

use workbench_engine::ai_settings::AiSettings;

use crate::accounts::{CODEX_ACCOUNT, MPC_ACCOUNT, has_key_for, key_for, service_called};
use crate::chatgpt;
use crate::key_store;
use crate::picked_card::PickedCard;
use crate::payloads::{self, AiChoice, AiPicks, ChangeAsked, HuntAsked, IdeaAsked, ListingAsked, MpcAsked, NewPlugin};
use crate::player::{LivePlayer, Played};
use crate::sample_player::SamplePlayer;
use crate::updates::Updating;
use crate::upload::UploadedFile;
use crate::workbench::Workbench;

const PROGRESS_EVENT: &str = "work-progress";
const FINISHED_EVENT: &str = "work-finished";
const CONTEXT_EVENT: &str = "work-context";
/// A program that prints the words on the clipboard, with what it is asked.
type ClipboardReader = (&'static str, &'static [&'static str]);
/// Each system has its own. Linux has several, depending on how its screen is drawn, so each is tried in turn.
const CLIPBOARD_READERS: &[ClipboardReader] = if cfg!(target_os = "macos") {
    &[("/usr/bin/pbpaste", &[])]
} else if cfg!(windows) {
    &[("powershell", &["-NoProfile", "-Command", "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-Clipboard -Raw"])]
} else {
    &[("wl-paste", &["--no-newline"]), ("xclip", &["-selection", "clipboard", "-o"]), ("xsel", &["--clipboard", "--output"])]
};
const CLIPBOARD_NOT_READ: &str = "The clipboard could not be read on this computer.";
const CHOOSE_A_PLUGINS_FOLDER: &str = "Choose the folder to keep your plugins in";
const CHOOSE_A_SOUND_FOR_THE_APP: &str = "Choose a sound file for the app, as from the MPC Sample's SD card";
const CHOOSE_A_TEST_SOUND: &str = "Choose a sound to play through the effect";
const SAVE_THE_PLUGIN_AS: &str = "Save this plugin to hand to somebody else";
const CHOOSE_A_PLUGIN_TO_IMPORT: &str = "Choose the plugin to bring in";
const CHOOSE_WHERE_THE_LISTING_GOES: &str = "Choose where to put the folder for Open MPC";
const CHOOSE_THE_MPC_SAMPLES_CARD: &str = "Choose your MPC Sample's SD card";
const PACKED_PLUGINS: &str = "MP Forge plugin";
const NOT_OF_THE_CATALOG: &str = "That is not a plugin of the Open MPC catalog.";
const ONLY_SDK_PLUGINS_ARE_TAKEN_IN: &str = "Only a plugin written with Akai's SDK, kept at the top of its repository, can be taken in as it is. You can still remix it.";
/// The program that shows a folder in the computer's own file manager.
#[cfg(target_os = "macos")]
const OPENS_FOLDERS: &str = "open";
#[cfg(target_os = "windows")]
const OPENS_FOLDERS: &str = "explorer";
#[cfg(all(unix, not(target_os = "macos")))]
const OPENS_FOLDERS: &str = "xdg-open";
/// The program that opens a web page in the musician's browser, and what it is told before the page's address.
/// On Windows the file manager will not do: given an address with a question in it, it shows a folder instead.
#[cfg(target_os = "macos")]
pub const OPENS_PAGES: (&str, &[&str]) = ("open", &[]);
#[cfg(target_os = "windows")]
pub const OPENS_PAGES: (&str, &[&str]) = ("rundll32", &["url.dll,FileProtocolHandler"]);
#[cfg(all(unix, not(target_os = "macos")))]
pub const OPENS_PAGES: (&str, &[&str]) = ("xdg-open", &[]);

#[tauri::command]
pub fn library(workbench: State<Workbench>) -> Value {
    let plugins = library::list_plugins(&workbench.plugins_dir());
    json!({ "plugins": plugins.iter().map(payloads::summary).collect::<Vec<Value>>() })
}

#[tauri::command]
pub fn open_plugin(name: String, workbench: State<Workbench>) -> Result<Value, String> {
    let plugin_dir = workbench.plugin_dir(&name)?;
    let summary = library::summary_of(&plugin_dir).ok_or_else(|| format!("There is no plugin called `{name}`."))?;
    Ok(json!({
        "summary": payloads::summary(&summary),
        "screen": library::screen_layout(&plugin_dir),
        "parameters": library::built_parameters(&plugin_dir),
        "versionsKept": history::versions_kept(&plugin_dir),
        "samples": SampleShelf::of_plugin(&plugin_dir).list().iter().map(payloads::sample).collect::<Vec<Value>>(),
        "pictures": pictures_on_screen(&plugin_dir),
        "arrangement": payloads::arrangement(&arranged_screen_of(&plugin_dir)),
        "look": look_of(&plugin_dir).unwrap_or_default().as_json(),
        "builtAt": built_screen_file(&plugin_dir).map(|screen_file| when_written(&screen_file)),
        "streams": payloads::streams(&online::streams_of(&plugin_dir).unwrap_or_default()),
    }))
}

fn pictures_on_screen(plugin_dir: &Path) -> Vec<Value> {
    let drawn = artwork::drawn_pictures(plugin_dir);
    let described = |picture: &Picture| {
        let drawn_at = when_written(&artwork::drawn_file_of(plugin_dir, &picture.name));
        payloads::picture(picture, drawn_at, picture_clock::turn_seconds_of(plugin_dir, picture))
    };
    drawn.iter().map(described).collect()
}

/// When a file was last written, in milliseconds. Zero when that cannot be told.
fn when_written(file: &Path) -> u64 {
    let written = fs::metadata(file).and_then(|details| details.modified()).ok();
    let since_1970 = written.and_then(|moment| moment.duration_since(UNIX_EPOCH).ok());
    since_1970.map_or(0, |duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

/// The drawn image of one picture: every frame, stacked top to bottom in one PNG.
#[tauri::command]
pub fn picture_image(name: String, picture: String, workbench: State<Workbench>) -> Result<Response, String> {
    let image = artwork::drawn_image(&workbench.plugin_dir(&name)?, &picture);
    image.map(Response::new).ok_or_else(|| format!("`{name}` has no picture called `{picture}`."))
}

/// One of the images the build put beside the plugin's screen: the artwork of its knobs, faders and switches.
#[tauri::command]
pub fn skin_image(name: String, file: String, workbench: State<Workbench>) -> Result<Response, String> {
    let image = built_skin_image(&workbench.plugin_dir(&name)?, &file);
    image.map(Response::new).ok_or_else(|| format!("`{name}` has no built image called `{file}`."))
}

/// Keeps a picture the musician wants the AI to see. The picture's bytes are the request body.
#[tauri::command]
pub fn add_attachment(request: Request<'_>, workbench: State<Workbench>) -> Result<String, String> {
    let file = UploadedFile::read_from(&request)?;
    let _claim = workbench.claim_to_change_by_hand(&file.plugin_name)?;
    workbench.maker()?.add_attachment(&file.plugin_name, file.bytes).map_err(explain)
}

/// Keeps a sound file the musician picked, for the plugin to play. The file's bytes are the request body.
#[tauri::command]
pub fn add_sample(request: Request<'_>, workbench: State<Workbench>) -> Result<Value, String> {
    let file = UploadedFile::read_from(&request)?;
    let _claim = workbench.claim_to_change_by_hand(&file.plugin_name)?;
    let kept = workbench.maker()?.add_sample(&file.plugin_name, &file.file_name, file.bytes).map_err(explain)?;
    Ok(payloads::sample(&kept))
}

#[tauri::command]
pub fn remove_sample(name: String, file_name: String, workbench: State<Workbench>) -> Result<(), String> {
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    workbench.maker()?.remove_sample(&name, &file_name).map_err(explain)
}

#[tauri::command]
pub fn start_plugin(request: NewPlugin, ai: AiChoice, picks: AiPicks, app: AppHandle, workbench: State<Workbench>) -> Result<(), String> {
    let ai = ai_for_a_job(ai, &picks)?;
    let kind = PluginKind::from_word(&request.kind).ok_or("A plugin is either an effect or an instrument.")?;
    let identity = PluginIdentity { name: request.name.clone(), maker: request.maker, kind };
    let plugin_request = PluginRequest { identity, description: request.description + &attachments::note_about(&request.pictures) };
    let plugin_name = request.name;
    run_in_background(&workbench, app, plugin_name, move |maker, connection, observer| maker.make(&plugin_request, connection, observer), ai)
}

#[tauri::command]
pub fn change_plugin(name: String, change: ChangeAsked, ai: AiChoice, picks: AiPicks, app: AppHandle, workbench: State<Workbench>) -> Result<(), String> {
    let ai = ai_for_a_job(ai, &picks)?;
    let plugin_name = name.clone();
    let earlier_talk = workbench.maker()?.earlier_talk(&name).map_err(explain)?;
    let change = change.words + &attachments::note_about(&change.pictures) + &earlier_talk;
    run_in_background(&workbench, app, plugin_name, move |maker, connection, observer| maker.revise(&name, &change, connection, observer), ai)
}

/// Asks the plugin architect for questions about an idea, before anything is built. The architect can only ask:
/// it is given no plugin and no tools that touch one. Takes a little while, so it runs off the main thread.
#[tauri::command(async)]
pub fn architect_questions(idea: IdeaAsked, ai: AiChoice, picks: AiPicks) -> Result<Vec<Value>, String> {
    let ai = chosen_ai(ai, &picks)?;
    let idea = idea.as_idea()?;
    let questions = asked_of(ai, &idea, &StopSignal::default())?;
    Ok(questions.iter().map(payloads::question).collect())
}

fn asked_of(ai: ChosenAi, idea: &architect::Idea, stop: &StopSignal) -> Result<Vec<Question>, String> {
    let asked = match ai {
        ChosenAi::ClaudeCode(settings) => architect::ask_claude_code(idea, settings, stop),
        ChosenAi::ClaudeDirect(settings) => architect::ask_model(&mut claude_signed_in(settings)?, idea),
        ChosenAi::Chat { provider, model } => architect::ask_model(&mut chat_model_for(provider, model)?, idea),
        ChosenAi::ChatGptPlan(model) => architect::ask_model(&mut PlanModel::new(chatgpt::KeptAccess, model), idea),
        ChosenAi::Codex(_) => return Err(NO_ARCHITECT_THROUGH_CODEX.to_string()),
    };
    asked.map_err(|problem| problem.to_string())
}

/// Whether the plugin has been built, which a guide needs, and whether the words of its guide are already
/// written for the controls and presets it has now.
#[tauri::command]
pub fn guide_state(name: String, workbench: State<Workbench>) -> Result<Value, String> {
    let plugin_dir = workbench.plugin_dir(&name)?;
    match guide::facts_of(&plugin_dir) {
        Ok(facts) => Ok(json!({ "built": true, "written": guide::kept_words(&plugin_dir, &facts).is_some() })),
        Err(GuideError::NotBuiltYet) => Ok(json!({ "built": false, "written": false })),
        Err(problem) => Err(problem.to_string()),
    }
}

/// Makes the plugin's guide, saves it in the plugin's folder as one page and opens it in the browser.
/// The AI is asked for the guide's words only when none are kept that fit the plugin, or when it is to write
/// them again. It is given what the plugin is and can only answer. Answers with where the page is and whether
/// the AI was asked. Takes a while when it is, so it runs off the main thread.
#[tauri::command(async)]
pub fn make_guide(name: String, write_again: bool, ai: AiChoice, picks: AiPicks, workbench: State<Workbench>) -> Result<Value, String> {
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    let plugin_dir = workbench.plugin_dir(&name)?;
    let facts = guide::facts_of(&plugin_dir).map_err(|problem| problem.to_string())?;
    let kept = if write_again { None } else { guide::kept_words(&plugin_dir, &facts) };
    let asked = kept.is_none();
    let words = match kept {
        Some(words) => words,
        None => words_written_and_kept(chosen_ai(ai, &picks)?, &plugin_dir, &facts)?,
    };
    let page = guide::write_page(&plugin_dir, &facts, &words).map_err(|problem| format!("The guide could not be saved: {problem}."))?;
    crate::accounts::open_in_the_browser(&page.to_string_lossy())?;
    Ok(json!({ "page": page, "asked": asked }))
}

fn words_written_and_kept(ai: ChosenAi, plugin_dir: &Path, facts: &Facts) -> Result<Words, String> {
    let material = guide::material_for(plugin_dir, facts);
    let written = match ai {
        ChosenAi::ClaudeCode(settings) => guide::ask_claude_code(&material, settings, &StopSignal::default()),
        ChosenAi::ClaudeDirect(settings) => guide::ask_model(&mut claude_signed_in(settings)?, &material),
        ChosenAi::Chat { provider, model } => guide::ask_model(&mut chat_model_for(provider, model)?, &material),
        ChosenAi::ChatGptPlan(model) => guide::ask_model(&mut PlanModel::new(chatgpt::KeptAccess, model), &material),
        ChosenAi::Codex(_) => return Err(NO_GUIDE_THROUGH_CODEX.to_string()),
    };
    let words = written.map_err(|problem| problem.to_string())?;
    guide::keep_words(plugin_dir, facts, &words).map_err(|problem| format!("The guide's words could not be kept: {problem}."))?;
    Ok(words)
}

/// Rolls the dice for "I'm feeling lucky": a theme, and whether the plugin is an effect or an instrument.
#[tauri::command]
pub fn roll_the_dice() -> Result<Value, String> {
    lucky_find::rolled().map(|roll| payloads::roll(&roll))
}

/// Sends the scout out for something new and interesting in the theme that was rolled, and returns the plugin
/// idea it made of its find. The scout can only look things up and answer: it is given no plugin and no tools
/// that touch one. Takes a while, so it runs off the main thread.
#[tauri::command(async)]
pub fn lucky_find(hunt: HuntAsked, ai: AiChoice, picks: AiPicks) -> Result<Value, String> {
    let ai = chosen_ai(ai, &picks)?;
    let hunt = hunt.as_hunt()?;
    found_by(ai, &hunt, &StopSignal::default()).map(|find| payloads::find(&find))
}

fn found_by(ai: ChosenAi, hunt: &Hunt, stop: &StopSignal) -> Result<Find, String> {
    let found = match ai {
        ChosenAi::ClaudeCode(settings) => lucky_find::ask_claude_code(hunt, settings, stop),
        ChosenAi::ClaudeDirect(settings) => lucky_find::ask_model(&mut claude_signed_in(settings)?, hunt),
        ChosenAi::Chat { provider, model } => lucky_find::ask_model(&mut chat_model_for(provider, model)?, hunt),
        ChosenAi::ChatGptPlan(model) => lucky_find::ask_model(&mut PlanModel::new(chatgpt::KeptAccess, model), hunt),
        ChosenAi::Codex(_) => return Err(NO_SCOUT_THROUGH_CODEX.to_string()),
    };
    found.map_err(|problem| problem.to_string())
}

/// Reads the Open MPC catalog, so the musician can pick a plugin to remix. Only the list is fetched. It is a
/// few megabytes, so it runs off the main thread.
#[tauri::command(async)]
pub fn open_mpc_catalog() -> Result<Vec<Value>, String> {
    Ok(open_mpc::catalog()?.iter().map(payloads::listed_for_remixing).collect())
}

/// What the makers of a plugin in the catalog wrote about it, for the musician to read and the builder to
/// work from. Nothing when the project has no README.
#[tauri::command(async)]
pub fn remix_readme(source: String) -> Result<Option<Value>, String> {
    let readme = open_mpc::readme_of(&source)?;
    Ok(readme.map(|readme| json!({ "toldToTheBuilder": open_mpc::readme_as_told_to_the_builder(&readme), "shown": readme })))
}

/// The picture of a catalog plugin's screen, to go along with the request as a picture the musician attached.
#[tauri::command(async)]
pub fn remix_screenshot(address: String) -> Result<Response, String> {
    open_mpc::screenshot_at(&address).map(Response::new)
}

fn claude_signed_in(settings: AiSettings) -> Result<Claude, String> {
    Claude::signed_in_with(key_store::kept(ANTHROPIC.word), settings).map_err(|error| error.to_string())
}

fn chat_model_for(provider: &'static Provider, model: ChosenModel) -> Result<ChatModel, String> {
    let key = key_for(provider);
    if key.is_none() && provider.needs_a_key() {
        return Err(format!("The key for {} could not be read from this computer's store for secrets.", provider.name));
    }
    Ok(ChatModel::new(provider, key, model))
}

/// Runs the safety check on a plugin as it stands. Takes several seconds, so it runs off the main thread.
#[tauri::command(async)]
pub fn check_plugin(name: String, workbench: State<Workbench>) -> Result<Value, String> {
    let _claim = workbench.claim_to_check(&name)?;
    readiness::fetch_what_is_missing(&workbench.sdk()?, &|_fetching| {})?;
    let report = workbench.maker()?.inspect(&name).map_err(explain)?;
    Ok(payloads::report(&report))
}

/// Plays the plugin in the simulator and returns the recording as sound-file bytes.
#[tauri::command(async)]
pub fn preview_plugin(name: String, workbench: State<Workbench>) -> Result<Response, String> {
    let sound_file = workbench.maker()?.record_preview(&name).map_err(explain)?;
    fs::read(sound_file).map(Response::new).map_err(|error| error.to_string())
}

/// Asks the AI's work on one plugin to stop. It gives up at its next step and reports that it was stopped.
#[tauri::command]
pub fn stop_work(name: String, workbench: State<Workbench>) {
    workbench.ask_job_to_stop_on(&name);
}

/// What has been said about a plugin, as it was last kept. A plugin that is still being made has nothing yet.
#[tauri::command]
pub fn conversation(name: String, workbench: State<Workbench>) -> Vec<Value> {
    workbench.maker().ok().and_then(|maker| maker.conversation(&name).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn keep_conversation(name: String, messages: Vec<Value>, workbench: State<Workbench>) -> Result<(), String> {
    workbench.maker()?.keep_conversation(&name, &messages).map_err(explain)
}

/// Every version of a plugin that has been kept, oldest first, for the musician to step through.
#[tauri::command]
pub fn versions(name: String, workbench: State<Workbench>) -> Result<Vec<Value>, String> {
    versions_of(&workbench.maker()?, &name)
}

fn versions_of(maker: &Maker, name: &str) -> Result<Vec<Value>, String> {
    let kept = maker.versions(name).map_err(explain)?;
    Ok(kept.iter().map(payloads::version).collect())
}

/// Puts a plugin back as one kept version holds it. What was there is kept first, so nothing is lost.
/// Answers whether there was a version with that number.
#[tauri::command]
pub fn go_to_version(name: String, number: usize, workbench: State<Workbench>) -> Result<bool, String> {
    let _claim = workbench.claim_to_check(&name)?;
    workbench.maker()?.go_to_version(&name, number).map_err(explain)
}

/// Throws one kept version of a plugin away for good, once the musician has said so. Answers with the versions left.
#[tauri::command]
pub fn forget_version(name: String, number: usize, workbench: State<Workbench>) -> Result<Vec<Value>, String> {
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    let maker = workbench.maker()?;
    maker.forget_version(&name, number).map_err(explain)?;
    versions_of(&maker, &name)
}

/// Folds a plugin's oldest versions away so that at most `most` are left, as the musician's settings ask.
/// Answers with how many went.
#[tauri::command]
pub fn fold_old_versions(name: String, most: usize, workbench: State<Workbench>) -> Result<usize, String> {
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    workbench.maker()?.fold_old_versions(&name, most).map_err(explain)
}

/// Sends a plugin to a removable drive of the musician's MPC. The safety check is run on the plugin as it stands
/// first, and a plugin it stops is not sent. The MPC's app is restarted only when the musician asked for that, this once.
#[tauri::command(async)]
pub fn send_to_mpc(name: String, mpc: MpcAsked, workbench: State<Workbench>) -> Result<Value, String> {
    let _claim = workbench.claim_to_check(&name)?;
    let restart_asked = mpc.restart;
    let mpc = mpc_of(&mpc)?;
    let maker = workbench.maker()?;
    match maker.send_to_mpc(&name, &mpc).map_err(explain)? {
        Sending::HeldBack(report) => Ok(json!({ "sent": false, "report": payloads::report(&report) })),
        Sending::Sent { report, onto_drive } => {
            let restart_problem = restart_asked.then(|| maker.restart_mpc_app(&mpc).map_err(explain).err()).flatten();
            let restarted = restart_asked && restart_problem.is_none();
            Ok(json!({ "sent": true, "report": payloads::report(&report), "drive": onto_drive, "restarted": restarted, "restartProblem": restart_problem }))
        }
    }
}

fn mpc_of(asked: &MpcAsked) -> Result<Mpc, String> {
    let reached = Mpc::at(&asked.address).and_then(|mpc| mpc.using_drive(asked.drive.as_deref())).map_err(|unusable| unusable.to_string())?;
    Ok(reached.with_password(key_store::kept(MPC_ACCOUNT)))
}

/// Which copy of MP Forge this is: its version, the commit it was built from with that commit's day, and how
/// it comes by a newer version. Copies of one version can differ, and the commit tells them apart.
#[tauri::command]
pub fn this_copy(app: AppHandle) -> Value {
    json!({ "version": app.package_info().version.to_string(), "builtFrom": option_env!("MP_FORGE_BUILT_FROM"), "updating": Updating::of_this_copy().word() })
}

/// The words on this computer's clipboard, for the app's own Paste.
#[tauri::command]
pub fn clipboard_words() -> Result<String, String> {
    CLIPBOARD_READERS.iter().find_map(words_printed_by).ok_or_else(|| CLIPBOARD_NOT_READ.to_string())
}

/// What the reader prints, when this computer has it and it could read the clipboard.
fn words_printed_by(reader: &ClipboardReader) -> Option<String> {
    let (program, asked) = reader;
    let printed = command_for(program).args(*asked).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    printed.status.success().then(|| String::from_utf8_lossy(&printed.stdout).into_owned())
}

/// Where the plugins are kept on this computer, and whether that folder can be found just now.
#[tauri::command]
pub fn plugins_folder(workbench: State<Workbench>) -> Value {
    payloads::plugins_folder(&workbench.plugins_dir())
}

/// Opens the plugins folder in the computer's own file manager.
#[tauri::command]
pub fn show_plugins_folder(workbench: State<Workbench>) -> Result<(), String> {
    let folder = workbench.plugins_dir();
    if !folder.is_dir() {
        return Err(format!("{} cannot be found just now.", folder.display()));
    }
    command_for(OPENS_FOLDERS).arg(&folder).spawn().map(drop).map_err(|problem| format!("The folder could not be opened: {problem}."))
}

/// Lets the musician pick a folder with the computer's own folder chooser. Nothing changes until they say
/// what should happen to the plugins they have. None when they picked nothing.
#[tauri::command(async)]
pub fn choose_plugins_folder(app: AppHandle, workbench: State<Workbench>) -> Result<Option<Value>, String> {
    let chooser = app.dialog().file().set_title(CHOOSE_A_PLUGINS_FOLDER).set_directory(nearest_folder_found(&workbench.plugins_dir()));
    let Some(picked) = chooser.blocking_pick_folder() else { return Ok(None) };
    let folder = picked.into_path().map_err(|problem| format!("That folder cannot be used: {problem}."))?;
    Ok(Some(payloads::plugins_folder(&folder)))
}

/// A chooser for one WAV or MP3 file, under the title that says what the file is for.
fn sound_file_chooser(app: &AppHandle, title: &str) -> tauri_plugin_dialog::FileDialogBuilder<tauri::Wry> {
    app.dialog().file().set_title(title).add_filter("WAV or MP3 sound", &["wav", "WAV", "mp3", "MP3"])
}

/// Lets the musician pick a sound file of their own to hear an effect work on. Only where the file is comes
/// back: the live player reads it when it is asked to play it, and it is never copied into a plugin.
/// None when they picked nothing.
#[tauri::command(async)]
pub fn choose_test_sound(app: AppHandle) -> Result<Option<Value>, String> {
    let Some(picked) = sound_file_chooser(&app, CHOOSE_A_TEST_SOUND).blocking_pick_file() else { return Ok(None) };
    let file = picked.into_path().map_err(|problem| format!("That file cannot be used: {problem}."))?;
    let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Some(json!({ "path": file, "name": name })))
}

/// Keeps the plugins in the chosen folder from now on, moving them there first when they are to come along.
/// The live player lets go of the plugin it holds, so its files are free to move.
#[tauri::command(async)]
pub fn keep_plugins_in(folder: String, bring_along: bool, workbench: State<Workbench>, player: State<LivePlayer>) -> Result<Value, String> {
    player.stop();
    let moved = workbench.keep_plugins_in(Path::new(&folder), bring_along)?;
    Ok(json!({ "moved": moved.moved, "leftBehind": moved.left_behind, "folder": payloads::plugins_folder(&workbench.plugins_dir()) }))
}

/// The chooser opens at the plugins folder, or at the nearest folder above it that is there.
fn nearest_folder_found(folder: &Path) -> PathBuf {
    folder.ancestors().find(|above| above.is_dir()).unwrap_or(folder).to_path_buf()
}

/// Gives a plugin another name. A plugin that is being worked on keeps its name until that has finished.
#[tauri::command]
pub fn rename_plugin(name: String, new_name: String, workbench: State<Workbench>) -> Result<(), String> {
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    workbench.maker()?.rename_plugin(&name, &new_name).map_err(explain)
}

/// Switches online plugins on or off: whether a plugin may bring sound in from the internet through MP Forge's
/// stream helper. It is experimental, and off until the musician switches it on in the settings.
#[tauri::command]
pub fn allow_online_plugins(allowed: bool, workbench: State<Workbench>) {
    workbench.allow_online_plugins(allowed);
}

/// Lets the musician choose where to save the plugin's project as one file, and packs it there. The file holds
/// what makes the plugin and nothing of its builds, its earlier versions or its conversation.
/// Answers with where it was saved, or with nothing when they chose nowhere.
#[tauri::command(async)]
pub fn export_plugin(name: String, app: AppHandle, workbench: State<Workbench>) -> Result<Option<PathBuf>, String> {
    let chooser = app.dialog().file().set_title(SAVE_THE_PLUGIN_AS).set_file_name(format!("{name}.{PROJECT_FILE_ENDING}"));
    let Some(picked) = chooser.add_filter(PACKED_PLUGINS, &[PROJECT_FILE_ENDING]).blocking_save_file() else { return Ok(None) };
    let file = picked.into_path().map_err(|problem| format!("That place cannot be used: {problem}."))?.with_extension(PROJECT_FILE_ENDING);
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    workbench.maker()?.export_project(&name, &file).map_err(explain)?;
    Ok(Some(file))
}

/// How the plugin would be listed in the Open MPC catalog, for the sheet that asks about it. Only a built
/// plugin can be listed.
#[tauri::command]
pub fn open_mpc_listing_state(name: String, workbench: State<Workbench>) -> Result<Value, String> {
    match workbench.maker()?.as_listed_on_open_mpc(&name) {
        Ok(listed) => Ok(payloads::as_it_would_be_listed(&listed)),
        Err(MakeError::NotListed(NotListed::NotBuiltYet)) => Ok(json!({ "built": false })),
        Err(problem) => Err(explain(problem)),
    }
}

/// Lets the musician choose a folder, and makes in it everything a listing in the Open MPC catalog needs: the
/// plugin's source with its guide as the README, the built plugin as a zip, and the file that asks for the
/// listing. The safety check is run on the plugin as it stands first, and a plugin it stops is not got ready.
/// Nothing is sent anywhere. Answers with nothing when they chose no folder.
#[tauri::command(async)]
pub fn list_on_open_mpc(name: String, listing: ListingAsked, app: AppHandle, workbench: State<Workbench>) -> Result<Option<Value>, String> {
    let listing = listing.as_listing()?;
    let Some(picked) = app.dialog().file().set_title(CHOOSE_WHERE_THE_LISTING_GOES).blocking_pick_folder() else { return Ok(None) };
    let into = picked.into_path().map_err(|problem| format!("That place cannot be used: {problem}."))?;
    let _claim = workbench.claim_to_check(&name)?;
    match workbench.maker()?.list_on_open_mpc(&name, &listing, &into).map_err(explain)? {
        Listed::HeldBack(report) => Ok(Some(json!({ "ready": false, "report": payloads::report(&report) }))),
        Listed::Ready { report, folder } => {
            let _ = command_for(OPENS_FOLDERS).arg(&folder).spawn();
            Ok(Some(json!({ "ready": true, "report": payloads::report(&report), "folder": folder })))
        }
    }
}

/// Lets the musician pick a plugin to bring in: a file packed by MP Forge, or the folder of a plugin's project.
/// Answers with where it is, or with nothing when they picked nothing. Nothing is read yet.
#[tauri::command(async)]
pub fn choose_plugin_to_import(as_a_folder: bool, app: AppHandle) -> Result<Option<PathBuf>, String> {
    let chooser = app.dialog().file().set_title(CHOOSE_A_PLUGIN_TO_IMPORT);
    let picked = if as_a_folder { chooser.blocking_pick_folder() } else { chooser.add_filter(PACKED_PLUGINS, &[PROJECT_FILE_ENDING]).blocking_pick_file() };
    picked.map(|picked| picked.into_path().map_err(|problem| format!("That cannot be used: {problem}."))).transpose()
}

/// Takes the picked project in as a new plugin, under `named` when a name is given. Nothing of it is run.
/// Answers with the new plugin's `name`, or with `nameTaken` when there is already a plugin of its name,
/// so the musician can pick another.
#[tauri::command(async)]
pub fn import_plugin(from: PathBuf, named: Option<String>, workbench: State<Workbench>) -> Result<Value, String> {
    match workbench.maker()?.import_project(&from, named.as_deref()) {
        Ok(name) => Ok(json!({ "name": name })),
        Err(NotTakenIn::NameTaken(name)) => Ok(json!({ "nameTaken": name })),
        Err(not_taken_in) => Err(not_taken_in.to_string()),
    }
}

/// Whether a catalog plugin's source has a project at its top, which is the only kind MP Forge can take in.
/// One small file is asked for, so the musician knows before they try.
#[tauri::command(async)]
pub fn open_mpc_source_holds_a_project(source: String) -> Result<bool, String> {
    Ok(open_mpc::manifest_of(&source)?.is_some())
}

/// Takes in a plugin of the Open MPC catalog as its makers wrote it, under `named` when a name is given. Its
/// source is fetched and nothing of it is run. Answers as `import_plugin` does.
#[tauri::command(async)]
pub fn import_from_open_mpc(original: Value, named: Option<String>, workbench: State<Workbench>) -> Result<Value, String> {
    let origin = Origin::said_in(&original).ok_or(NOT_OF_THE_CATALOG)?;
    match workbench.maker()?.import_from_open_mpc(&origin, named.as_deref()) {
        Ok(name) => Ok(json!({ "name": name, "credit": origin.credit() })),
        Err(NotTakenIn::NameTaken(name)) => Ok(json!({ "nameTaken": name })),
        Err(NotTakenIn::NotAProject(why)) => Err(format!("MP Forge cannot build {} from its source: {why} {ONLY_SDK_PLUGINS_ARE_TAKEN_IN}", origin.name)),
        Err(not_taken_in) => Err(not_taken_in.to_string()),
    }
}

/// Moves a plugin's folder to the Trash. A plugin that is being worked on stays until that has finished.
#[tauri::command]
pub fn delete_plugin(name: String, workbench: State<Workbench>) -> Result<(), String> {
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    workbench.maker()?.delete_plugin(&name).map_err(explain)
}

/// Gets ready for the musician to arrange the screen by hand: keeps a version, so GO BACK undoes the whole session.
#[tauri::command]
pub fn begin_editing(name: String, workbench: State<Workbench>) -> Result<(), String> {
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    workbench.maker()?.begin_editing(&name).map_err(explain)
}

/// Keeps what the musician did to the look by hand. Answers with the screen as it is now arranged, and
/// with its layout, which changes when a control was put on a page or taken off one.
/// A look that would not fit the MPC's screen is refused, with the reason in plain words, and nothing is kept.
#[tauri::command]
pub fn keep_look(name: String, look: Value, workbench: State<Workbench>) -> Result<Value, String> {
    let look = look_from(&look)?;
    let _claim = workbench.claim_to_change_by_hand(&name)?;
    let pages = workbench.maker()?.keep_look(&name, &look).map_err(explain)?;
    Ok(json!({ "arrangement": payloads::arrangement(&pages), "screen": library::screen_layout(&workbench.plugin_dir(&name)?) }))
}

/// Keeps a picture the musician brought into the editor as artwork. The picture's bytes are the request body.
/// Answers with the name it was kept under and its size in its own pixels.
#[tauri::command]
pub fn add_artwork(request: Request<'_>, workbench: State<Workbench>) -> Result<Value, String> {
    let file = UploadedFile::read_from(&request)?;
    let _claim = workbench.claim_to_change_by_hand(&file.plugin_name)?;
    let kept_as = workbench.maker()?.add_artwork(&file.plugin_name, file.bytes).map_err(explain)?;
    art_details(&workbench.plugin_dir(&file.plugin_name)?, &kept_as)
}

/// The size, in its own pixels, of a picture the musician brought in as artwork.
#[tauri::command]
pub fn art_size(name: String, file: String, workbench: State<Workbench>) -> Result<Value, String> {
    art_details(&workbench.plugin_dir(&name)?, &file)
}

fn art_details(plugin_dir: &Path, file: &str) -> Result<Value, String> {
    let picture = ArtShelf::of_plugin(plugin_dir).decoded(file).ok_or_else(|| format!("The plugin has no artwork picture called `{file}`."))?;
    Ok(json!({ "file": file, "width": picture.width(), "height": picture.height() }))
}

/// One layer of artwork drawn by itself, exactly as it will look on the MPC, for the editor to show.
/// The layer is the one handed over, so it can be shown before the look that holds it is kept.
#[tauri::command(async)]
pub fn layer_image(name: String, layer: Value, workbench: State<Workbench>) -> Result<Response, String> {
    let layer = layer_from(&layer)?;
    page_art::drawn_alone(&workbench.plugin_dir(&name)?, &layer).map(Response::new)
}

/// All the artwork behind one page as the single picture the MPC will show.
#[tauri::command(async)]
pub fn page_art_image(name: String, page: String, workbench: State<Workbench>) -> Result<Response, String> {
    let image = artwork::page_art_image(&workbench.plugin_dir(&name)?, &page);
    image.map(Response::new).ok_or_else(|| format!("Page `{page}` of `{name}` has no artwork behind it."))
}

/// The fonts on this computer, for the editor's words. Finding them the first time takes a moment.
#[tauri::command(async)]
pub fn font_names() -> Vec<String> {
    fonts::font_names()
}

/// One of the pictures the musician attached, to show beside the message it came with.
#[tauri::command]
pub fn attachment_image(name: String, file: String, workbench: State<Workbench>) -> Result<Response, String> {
    let image = workbench.maker()?.attachment(&name, &file).map_err(explain)?;
    image.map(Response::new).ok_or_else(|| format!("`{name}` has no attached picture called `{file}`."))
}

/// What MP Forge needs on this computer besides itself, and whether each thing is there. Looking takes a moment.
#[tauri::command(async)]
pub fn what_is_needed(workbench: State<Workbench>) -> Result<Vec<Value>, String> {
    Ok(readiness::needs_on_this_computer(&workbench.sdk()?).iter().map(payloads::need).collect())
}

/// Fetches whichever of Python and Akai's build tools this computer lacks, so the first plugin does not wait
/// on them. Takes a minute or two.
#[tauri::command(async)]
pub fn fetch_what_is_missing(workbench: State<Workbench>) -> Result<(), String> {
    readiness::fetch_what_is_missing(&workbench.sdk()?, &|_fetching| {})
}

/// Starts the live player for a built plugin. What it says arrives as `player-event`s.
#[tauri::command]
pub fn player_start(name: String, run: u32, app: AppHandle, workbench: State<Workbench>, player: State<LivePlayer>) -> Result<(), String> {
    let plugin_library = workbench.maker()?.built_library(&name).map_err(explain)?;
    let played = Played { plugin_library: &plugin_library, may_go_online: workbench.online_plugins_are_allowed() };
    player.start(run, &played, app)
}

/// Passes one request to the live player: turn a control, switch the sound, play a note, or feed an effect
/// another sound.
#[tauri::command]
pub fn player_tell(request: Value, player: State<LivePlayer>) -> Result<(), String> {
    player.tell(&request)
}

#[tauri::command]
pub fn player_stop(player: State<LivePlayer>) {
    player.stop();
}

/// Builds a Sample app for this computer from its code as it stands, and starts it in a player of its own
/// with the sound files the musician has added to it.
#[tauri::command(async)]
pub fn sample_app_start(name: String, workbench: State<Workbench>, player: State<SamplePlayer>) -> Result<(), String> {
    let app_dir = workbench.maker()?.build_sample_app(&name).map_err(explain)?;
    player.start(&name, &app_dir)
}

/// Lets the musician pick their MPC Sample's SD card, or any folder, and makes MP Forge Link there: the small
/// program that turns the device into the screen and the controls of an app that runs here. On a card that has
/// the boot menu it goes where the boot menu looks. Nothing else on the card is touched. Answers with where it
/// was made, or with nothing when they picked no folder.
#[tauri::command(async)]
pub fn make_link_for_mpc_sample(app: AppHandle, workbench: State<Workbench>) -> Result<Option<Value>, String> {
    let Some(picked) = app.dialog().file().set_title(CHOOSE_THE_MPC_SAMPLES_CARD).blocking_pick_folder() else { return Ok(None) };
    let picked = picked.into_path().map_err(|problem| format!("That place cannot be used: {problem}."))?;
    let made = link_program::write_for_the_card_picked(&workbench.sdk()?, &picked, link_program::TIME_ALLOWED_TO_BUILD);
    made.map(|made| Some(payloads::made_for_the_card(&made))).map_err(|not_made| format!("MP Forge Link was not made. {not_made}"))
}

/// Lets the musician pick their MPC Sample's SD card, or any folder, for an app to be written to. The place is
/// kept here for `put_sample_app_on_card` and never handed to the screen. Answers with whether they picked one.
#[tauri::command(async)]
pub fn choose_card_for_sample_app(app: AppHandle, picked_card: State<PickedCard>) -> Result<bool, String> {
    let Some(picked) = app.dialog().file().set_title(CHOOSE_THE_MPC_SAMPLES_CARD).blocking_pick_folder() else { return Ok(false) };
    picked_card.keep(picked.into_path().map_err(|problem| format!("That place cannot be used: {problem}."))?);
    Ok(true)
}

/// Writes a Sample app, built for the MPC Sample, onto the card the musician just picked. The safety check is
/// run on the app as it stands first, and an app it stops is not written. On a card that has the boot menu
/// the app goes where the boot menu looks. Nothing else on the card is touched.
#[tauri::command(async)]
pub fn put_sample_app_on_card(name: String, workbench: State<Workbench>, picked_card: State<PickedCard>) -> Result<Value, String> {
    let picked = picked_card.take().ok_or("Choose the card first.")?;
    let _claim = workbench.claim_to_check(&name)?;
    match workbench.maker()?.put_sample_app_on_card(&name, &picked).map_err(explain)? {
        PutOnACard::HeldBack(report) => Ok(json!({ "done": false, "report": payloads::report(&report) })),
        PutOnACard::Written { report, made } => Ok(json!({ "done": true, "report": payloads::report(&report), "made": payloads::made_for_the_card(&made) })),
    }
}

/// Sends a Sample app, built for the MPC Sample, to a linked MPC Sample over MP Forge Link, which puts it on
/// the card in the device and runs it. The safety check is run on the app as it stands first, and an app it
/// stops is not sent.
#[tauri::command(async)]
pub fn send_sample_app_by_link(name: String, workbench: State<Workbench>) -> Result<Value, String> {
    let _claim = workbench.claim_to_check(&name)?;
    match workbench.maker()?.send_sample_app_by_link(&name, &Doors::of_the_mpc_sample()).map_err(explain)? {
        SentToTheSample::HeldBack(report) => Ok(json!({ "done": false, "report": payloads::report(&report) })),
        SentToTheSample::Sent(report) => Ok(json!({ "done": true, "report": payloads::report(&report) })),
    }
}

/// Shows the blacksmith on the drawing of the MPC Sample, and on a real one that is linked, in place of an app
/// that is out of reach while it is written, built or checked.
#[tauri::command]
pub fn sample_smith_start(name: String, keeps_still: bool, player: State<SamplePlayer>) -> Result<(), String> {
    player.show_the_smith(&name, keeps_still)
}

/// Lets the musician pick a sound file on the computer for an app that has opened its page for picking one
/// from the SD card, which a computer has not got. The app's player is told which file, and reads it, or that
/// they picked none. Where the file is never passes through the screen.
#[tauri::command(async)]
pub fn sample_app_pick_sound(name: String, app: AppHandle, player: State<SamplePlayer>) -> Result<(), String> {
    let picked = sound_file_chooser(&app, CHOOSE_A_SOUND_FOR_THE_APP).blocking_pick_file().and_then(|picked| picked.into_path().ok());
    player.tell(&name, &json!({ "picked": picked }))
}

/// Passes on one thing the musician touched on the drawing of the MPC Sample.
#[tauri::command]
pub fn sample_app_tell(name: String, input: Value, player: State<SamplePlayer>) -> Result<(), String> {
    player.tell(&name, &input)
}

/// What the Sample app has shown since the screen last asked: its picture and its lights, as bytes. Nothing at
/// all when that has not changed.
#[tauri::command]
pub fn sample_app_shown(name: String, player: State<SamplePlayer>) -> Result<Response, String> {
    Ok(Response::new(player.newly_shown(&name)?.unwrap_or_default()))
}

#[tauri::command]
pub fn sample_app_stop(player: State<SamplePlayer>) {
    player.stop();
}

/// Which AI does the work, and how it is reached.
enum ChosenAi {
    /// A Claude model through the musician's own Claude Code.
    ClaudeCode(AiSettings),
    /// A Claude model straight from Anthropic.
    ClaudeDirect(AiSettings),
    /// A model from any other service, which needs the musician's key for that service.
    Chat { provider: &'static Provider, model: ChosenModel },
    /// One of OpenAI's models through the musician's own Codex, signed in with their ChatGPT account.
    Codex(CodexChoices),
    /// One of OpenAI's models on the musician's own ChatGPT plan, signed in to on OpenAI's page.
    ChatGptPlan(ChosenModel),
}

const CHATGPT_NOT_SIGNED_IN: &str = "Sign in with ChatGPT first: pick one of its models from the menu at the top.";

const CODEX_NOT_SIGNED_IN: &str = "Codex is not signed in. Run `codex login` in a terminal and sign in with your ChatGPT account, then try again.";
const CODEX_NOT_INSTALLED: &str = "Codex is not installed on this computer. Pick one of the ChatGPT models from the menu at the top to see how to set it up.";
const NO_SCOUT_THROUGH_CODEX: &str = "The dice cannot be rolled through the ChatGPT sign-in yet. Pick another model from the menu at the top and roll again.";
const NO_GUIDE_THROUGH_CODEX: &str = "A guide's words cannot be written through the ChatGPT sign-in yet. Pick another model for the guide.";
const NO_ARCHITECT_THROUGH_CODEX: &str = "The plugin architect cannot ask its questions through the ChatGPT sign-in yet. Pick another model for the questions, or untick the architect and build straight away.";

fn codex_if_ready(picks: &AiPicks) -> Result<ChosenAi, String> {
    match codex::sign_in() {
        SignIn::SignedIn => Ok(ChosenAi::Codex(picks.as_codex_choices()?)),
        SignIn::NotSignedIn => Err(CODEX_NOT_SIGNED_IN.to_string()),
        SignIn::NotInstalled => Err(CODEX_NOT_INSTALLED.to_string()),
    }
}

/// Reads what was picked on screen. A service the musician has no key for is refused here, before any work starts.
fn chosen_ai(route: AiChoice, picks: &AiPicks) -> Result<ChosenAi, String> {
    if picks.provider.as_deref() == Some(CODEX_ACCOUNT) {
        return codex_if_ready(picks);
    }
    if picks.provider.as_deref() == Some(chatgpt::ACCOUNT) {
        return if chatgpt::is_signed_in() { Ok(ChosenAi::ChatGptPlan(picks.as_chosen_model()?)) } else { Err(CHATGPT_NOT_SIGNED_IN.to_string()) };
    }
    let provider = service_called(picks.provider.as_deref().unwrap_or(ANTHROPIC.word))?;
    if provider.is_anthropic() {
        let settings = picks.as_settings()?;
        return Ok(if route == AiChoice::ClaudeCode { ChosenAi::ClaudeCode(settings) } else { ChosenAi::ClaudeDirect(settings) });
    }
    if provider.needs_a_key() && !has_key_for(provider) {
        return Err(format!("Add your {} key first: pick one of its models from the menu at the top.", provider.name));
    }
    Ok(ChosenAi::Chat { provider, model: picks.as_chosen_model()? })
}

/// Which AI does a job, and when its conversation is tidied to make room.
struct AiForAJob {
    ai: ChosenAi,
    compact_at: Option<u64>,
}

impl AiForAJob {
    /// How much room the conversation has, for a model on this computer. It is asked for when the work
    /// starts, because it follows from this computer's settings and memory, which no list of models says.
    fn room_on_this_computer(&self) -> Option<u64> {
        match &self.ai {
            ChosenAi::Chat { provider, model } => provider.room_given_to(&model.id),
            _ => None,
        }
    }
}

fn ai_for_a_job(route: AiChoice, picks: &AiPicks) -> Result<AiForAJob, String> {
    Ok(AiForAJob { ai: chosen_ai(route, picks)?, compact_at: picks.compact_at })
}

type Job = dyn FnOnce(&Maker, Connection, &mut dyn FnMut(Event)) -> Result<Delivery, MakeError> + Send;

/// Tells the screen how the AI's work on one plugin is going. Several plugins are worked on at once, so
/// all that is told names the plugin it is about.
struct NewsOfTheWork {
    app: AppHandle,
    plugin_name: String,
}

impl NewsOfTheWork {
    fn tell(&self, event: &str, mut news: Value) {
        news["plugin"] = json!(self.plugin_name);
        let _ = self.app.emit(event, news);
    }
}

/// Starts the AI's work on its own thread. Refuses if it has as many plugins in hand as it takes at once.
fn run_in_background(
    workbench: &Workbench,
    app: AppHandle,
    plugin_name: String,
    job: impl FnOnce(&Maker, Connection, &mut dyn FnMut(Event)) -> Result<Delivery, MakeError> + Send + 'static,
    ai: AiForAJob,
) -> Result<(), String> {
    let claim = workbench.claim_for_ai_work(&plugin_name)?;
    let sdk = workbench.sdk()?;
    let maker = workbench.maker()?.stopped_by(workbench.stop_signal_for_job_on(&plugin_name));
    let news = NewsOfTheWork { app, plugin_name };
    thread::spawn(move || {
        let outcome = with_all_a_build_needs(&sdk, &news).and_then(|()| {
            let room = ai.room_on_this_computer();
            tell_of_little_room(room, &news);
            run_job(&maker.compacting_at(ollama::tidied_at(room, ai.compact_at)), Box::new(job), ai.ai, &news)
        });
        let message = match outcome {
            Ok(delivery) => payloads::finished(&news.plugin_name, &delivery),
            Err(explanation) => payloads::failed(&news.plugin_name, &explanation),
        };
        drop(claim);
        news.tell(FINISHED_EVENT, message);
    });
    Ok(())
}

/// A build never starts without Python and Akai's build tools. Whichever has gone missing since the app last
/// looked is fetched first, and the musician is told what the wait is for.
fn with_all_a_build_needs(sdk: &Sdk, news: &NewsOfTheWork) -> Result<(), String> {
    readiness::fetch_what_is_missing(sdk, &|fetching| news.tell(PROGRESS_EVENT, payloads::fetching(fetching)))
}

/// A model on this computer that was given little room loses the start of a long piece of work. The
/// musician is told so before the work begins, with what to change.
fn tell_of_little_room(room: Option<u64>, news: &NewsOfTheWork) {
    if let Some(said) = room.and_then(ollama::said_of_little_room) {
        news.tell(PROGRESS_EVENT, payloads::step(&said));
    }
}

fn run_job(maker: &Maker, job: Box<Job>, ai: ChosenAi, news: &NewsOfTheWork) -> Result<Delivery, String> {
    let mut report_progress = |event: Event| {
        if let Some(line) = payloads::progress(&event) {
            news.tell(PROGRESS_EVENT, line);
        }
        if let Some(held) = payloads::context(&event) {
            news.tell(CONTEXT_EVENT, held);
        }
    };
    match ai {
        ChosenAi::ClaudeCode(settings) => {
            let this_program = env::current_exe().map_err(|error| error.to_string())?;
            let connection = Connection::ClaudeCode { tool_server_program: &this_program, settings };
            job(maker, connection, &mut report_progress).map_err(explain)
        }
        ChosenAi::ClaudeDirect(settings) => {
            let mut claude = claude_signed_in(settings)?;
            job(maker, Connection::Model(&mut claude), &mut report_progress).map_err(explain)
        }
        ChosenAi::Chat { provider, model } => {
            let mut chat_model = chat_model_for(provider, model)?;
            job(maker, Connection::Model(&mut chat_model), &mut report_progress).map_err(explain)
        }
        ChosenAi::Codex(choices) => {
            let this_program = env::current_exe().map_err(|error| error.to_string())?;
            job(maker, Connection::Codex { tool_server_program: &this_program, choices }, &mut report_progress).map_err(explain)
        }
        ChosenAi::ChatGptPlan(model) => {
            let mut plan_model = PlanModel::new(chatgpt::KeptAccess, model);
            job(maker, Connection::Model(&mut plan_model), &mut report_progress).map_err(explain)
        }
    }
}

fn explain(error: MakeError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    /// These tests are built in a checkout, where git is at hand, so the note is always there to read.
    #[test]
    fn a_copy_built_in_a_checkout_knows_the_commit_it_was_built_from_and_its_day() {
        let built_from = option_env!("MP_FORGE_BUILT_FROM").expect("the build made no note of its commit");

        let (commit, day) = built_from.split_once(" of ").expect("the note names a commit and a day");
        assert!(commit.len() >= 7 && commit.chars().all(|letter| letter.is_ascii_hexdigit()), "{commit}");
        assert!(day.len() == 10 && day.split('-').count() == 3, "{day}");
    }
}
