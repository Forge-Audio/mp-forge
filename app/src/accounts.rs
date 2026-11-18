//! The musician's accounts with AI services, and their MPC's password: which are set up, setting one up,
//! and taking one away. The screen is told whether a key is kept, never what it is.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};

use workbench_engine::agent::ModelError;
use workbench_engine::browser_sign_in::{self, Attempt, Patience};
use workbench_engine::claude_code::{self, Standing};
use workbench_engine::codex::{self, SignIn};
use workbench_engine::process::command_for;
use workbench_engine::providers::{OfferedModel, Provider, every_provider, provider_called};

use crate::chatgpt;
use crate::commands::OPENS_PAGES;
use crate::key_store;

/// The name the MPC's password is kept under.
pub const MPC_ACCOUNT: &str = "mpc";

/// The word the screen knows the ChatGPT sign-in by. It is reached through the musician's own Codex, with no key.
pub const CODEX_ACCOUNT: &str = "codex";
const OPENAI: &str = "openai";

/// Where the musician sees and changes how much of their ChatGPT plan MP Forge may use.
const CHATGPT_USAGE_PAGE: &str = "https://chatgpt.com/settings/usage";
const ABOUT_THE_CHATGPT_PLAN: [&str; 4] = [
    "This needs a paid ChatGPT plan: Plus or Pro. Without one, OpenAI's page says it cannot continue, and there is nothing to connect.",
    "What MP Forge asks of the AI is counted against your plan, and MP Forge charges nothing for it.",
    "MP Forge opens OpenAI's own page in your browser. Sign in there and allow MP Forge to use your plan. You never type your password into MP Forge.",
    "Your requests and your plugin's files go to OpenAI under your account. The sign-in is kept in this computer's own store for secrets and sent nowhere else.",
];

/// Set when the musician stops waiting for a sign-in in their browser. One sign-in is waited for at a time.
static STOPPED_SIGNING_IN: AtomicBool = AtomicBool::new(false);

/// Every AI service the workbench can reach, and whether the musician has a key for it or is signed in to it.
#[tauri::command]
pub fn ai_accounts() -> Vec<Value> {
    every_provider().flat_map(accounts_listed_at).collect()
}

/// The two ways into ChatGPT are listed straight after OpenAI, whose models they reach.
fn accounts_listed_at(provider: &Provider) -> Vec<Value> {
    let its_own = account_of(provider);
    if provider.word == OPENAI { vec![its_own, chatgpt_account(), codex_account()] } else { vec![its_own] }
}

/// The musician's own ChatGPT plan, signed in to on OpenAI's page. There is no key to paste or to show.
fn chatgpt_account() -> Value {
    json!({
        "word": chatgpt::ACCOUNT,
        "name": "ChatGPT (your plan)",
        "hasKey": chatgpt::is_signed_in(),
        "keysAreMadeAt": "",
        "listsModelsWithoutAKey": false,
        "signsInWithBrowser": true,
        "takesNoKey": true,
        "usesChatGptPlan": true,
        "signInButton": "CONTINUE WITH CHATGPT",
        "aboutSigningIn": ABOUT_THE_CHATGPT_PLAN,
    })
}

/// The ChatGPT sign-in is Codex's own. The workbench only asks Codex whether it is signed in.
fn codex_account() -> Value {
    let sign_in = codex::sign_in();
    let standing = match sign_in {
        SignIn::SignedIn => "signed-in",
        SignIn::NotSignedIn => "not-signed-in",
        SignIn::NotInstalled => "not-installed",
    };
    json!({
        "word": CODEX_ACCOUNT,
        "name": "ChatGPT (Codex sign-in)",
        "hasKey": sign_in == SignIn::SignedIn,
        "signsIn": true,
        "signIn": standing,
        "keysAreMadeAt": "",
        "listsModelsWithoutAKey": false,
    })
}

fn account_of(provider: &Provider) -> Value {
    json!({
        "word": provider.word,
        "name": provider.name,
        "hasKey": has_key_for(provider),
        "keysAreMadeAt": provider.keys_are_made_at,
        "listsModelsWithoutAKey": provider.lists_models_without_a_key,
        "signsInWithBrowser": provider.signs_in_with_browser,
        "runsOnThisComputer": provider.runs_on_this_computer,
        "sellsWebLookups": provider.sells_web_lookups(),
    })
}

/// Signs the musician in on the service's own web page, opened in their browser, and keeps the key the service
/// then gives. The screen is never shown the key. It waits for the musician, so it runs off the main thread.
#[tauri::command(async)]
pub fn sign_in_with_browser(account: String) -> Result<(), String> {
    STOPPED_SIGNING_IN.store(false, Ordering::Relaxed);
    if account == chatgpt::ACCOUNT {
        return chatgpt::sign_in(&open_in_the_browser, &patience_with_the_browser());
    }
    let provider = service_called(&account)?;
    let pages = browser_sign_in::pages_of(provider).ok_or_else(|| format!("{} has no sign-in page MP Forge can use. Paste a key for it.", provider.name))?;
    let attempt = Attempt::begin()?;
    open_in_the_browser(&attempt.page_to_open(pages))?;
    let key = attempt.key_once_approved(pages, &patience_with_the_browser())?;
    key_store::keep(provider.word, &key)
}

/// Whether Claude Code can do MP Forge's work on this computer: installed, new enough and signed in. Claude Code
/// is asked itself, which takes a moment, so this runs off the main thread.
#[tauri::command(async)]
pub fn claude_code_standing() -> &'static str {
    match claude_code::standing() {
        Standing::Ready => "ready",
        Standing::NotSignedIn => "not-signed-in",
        Standing::TooOld => "too-old",
        Standing::NotInstalled => "not-installed",
    }
}

#[tauri::command]
pub fn stop_signing_in() {
    STOPPED_SIGNING_IN.store(true, Ordering::Relaxed);
}

/// The browser is waited on until the musician says to stop, or for as long as a sign-in can take.
fn patience_with_the_browser() -> Patience<'static> {
    Patience { limit: browser_sign_in::LONGEST_WAIT, has_given_up: &|| STOPPED_SIGNING_IN.load(Ordering::Relaxed) }
}

pub fn open_in_the_browser(page: &str) -> Result<(), String> {
    let (program, told_first) = OPENS_PAGES;
    command_for(program).args(told_first).arg(page).spawn().map(drop).map_err(|problem| format!("Your browser could not be opened: {problem}."))
}

/// Keeps a key the musician typed in, in this computer's keychain.
#[tauri::command]
pub fn keep_key(account: String, key: String) -> Result<(), String> {
    key_store::keep(known_account(&account)?, &key)
}

#[tauri::command(async)]
pub fn forget_key(account: String) -> Result<(), String> {
    if account == chatgpt::ACCOUNT {
        return chatgpt::sign_out();
    }
    key_store::forget(known_account(&account)?)
}

/// Opens the page where the musician manages how much of their ChatGPT plan MP Forge may use.
#[tauri::command]
pub fn open_chatgpt_usage() -> Result<(), String> {
    open_in_the_browser(CHATGPT_USAGE_PAGE)
}

#[tauri::command]
pub fn mpc_password_is_kept() -> bool {
    key_store::holds(MPC_ACCOUNT)
}

/// The models a service offers today, asked of the service itself. Takes a moment, so it runs off the main thread.
#[tauri::command(async)]
pub fn models_offered(provider: String) -> Result<Vec<Value>, String> {
    let signed_in_to = match provider.as_str() {
        CODEX_ACCOUNT => Some(codex::models_offered()),
        chatgpt::ACCOUNT => Some(chatgpt::models_offered()),
        _ => None,
    };
    if let Some(offered) = signed_in_to {
        return Ok(offered.map_err(|error| error.to_string())?.iter().map(offered_model).collect());
    }
    let service = service_asked_for_its_models(&provider)?;
    let offered = models_of_service(service).map_err(|error| why_not_listed(service, error))?;
    Ok(offered.iter().map(offered_model).collect())
}

/// Why a list could not be had. Models on this computer are not "a service that could not be reached":
/// what stands in the way is said as it is.
fn why_not_listed(provider: &Provider, error: ModelError) -> String {
    match error {
        ModelError::Unreachable(cause) if provider.runs_on_this_computer => cause,
        other => other.to_string(),
    }
}

/// Claude's models are named in the app itself. Anthropic's list is asked for in another way than every other
/// service's, so it is not asked for at all.
fn service_asked_for_its_models(word: &str) -> Result<&'static Provider, String> {
    let provider = service_called(word)?;
    if provider.is_anthropic() {
        return Err("Claude's models are the ones MP Forge names itself. Anthropic is not asked for a list.".to_string());
    }
    Ok(provider)
}

fn models_of_service(provider: &Provider) -> Result<Vec<OfferedModel>, ModelError> {
    provider.models_offered(key_for(provider).as_deref())
}

fn offered_model(model: &OfferedModel) -> Value {
    json!({ "id": model.id, "name": model.name, "seesPictures": model.sees_pictures, "contextWindow": model.context_window, "efforts": model.efforts })
}

pub fn service_called(word: &str) -> Result<&'static Provider, String> {
    provider_called(word).ok_or_else(|| format!("`{word}` is not an AI service MP Forge knows."))
}

/// The key the musician gave the workbench wins. Failing that, one set in the environment is used.
pub fn key_for(provider: &Provider) -> Option<String> {
    key_store::kept(provider.word).or_else(|| provider.key_in_environment())
}

pub fn has_key_for(provider: &Provider) -> bool {
    key_store::holds(provider.word) || provider.key_in_environment().is_some()
}

/// Secrets are only ever kept under the names the workbench itself uses.
fn known_account(account: &str) -> Result<&'static str, String> {
    let with_a_key = every_provider().filter(|provider| provider.needs_a_key()).map(|provider| provider.word);
    let known = with_a_key.chain([MPC_ACCOUNT]).find(|known| *known == account);
    known.ok_or_else(|| format!("`{account}` is not something MP Forge keeps a key for."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_only_kept_under_a_name_the_workbench_uses() {
        assert_eq!(known_account("openrouter"), Ok("openrouter"));
        assert_eq!(known_account(MPC_ACCOUNT), Ok(MPC_ACCOUNT));
        assert!(known_account("somebody-elses-service").is_err());
        assert!(known_account("ollama").is_err(), "models on this computer have no key to keep");
    }

    #[test]
    fn anthropic_is_never_asked_for_a_list_of_models() {
        assert!(service_asked_for_its_models("anthropic").is_err());
        assert!(service_asked_for_its_models("openrouter").is_ok());
        assert!(models_offered("anthropic".to_string()).is_err());
    }

    #[test]
    fn the_screen_is_told_of_every_service_and_never_shown_a_key() {
        let accounts = ai_accounts();

        assert_eq!(accounts[0]["word"], "anthropic");
        assert_eq!(accounts.len(), 10);
        assert_eq!(accounts[1]["word"], "openai");
        assert_eq!((accounts[2]["word"].clone(), accounts[2]["signInButton"].clone()), (json!(chatgpt::ACCOUNT), json!("CONTINUE WITH CHATGPT")));
        assert_eq!(accounts[3]["word"], CODEX_ACCOUNT);
        assert_eq!(accounts[3]["signsIn"], true);
        assert!(accounts.iter().all(|account| account["hasKey"].is_boolean() && account.get("key").is_none()));
        assert_eq!(accounts[9]["word"], "ollama");
        assert_eq!((accounts[9]["runsOnThisComputer"].clone(), accounts[8]["signsInWithBrowser"].clone()), (json!(true), json!(true)));
        assert!(!accounts.iter().any(|account| account["word"] == "huggingface"));
        assert_eq!(accounts.iter().filter(|account| account["sellsWebLookups"] == true).map(|account| account["word"].clone()).collect::<Vec<_>>(), ["openrouter"]);
    }

    #[test]
    fn only_a_service_with_a_sign_in_page_is_signed_in_to_in_the_browser() {
        let refusal = sign_in_with_browser("openai".to_string()).unwrap_err();

        assert!(refusal.contains("no sign-in page"), "{refusal}");
        assert!(sign_in_with_browser("somebody-elses-service".to_string()).is_err());
    }

    #[test]
    fn what_stops_this_computers_models_being_listed_is_said_as_it_is() {
        let not_running = ModelError::Unreachable("Ollama is not running on this computer.".to_string());
        let down = ModelError::Unreachable("no route".to_string());

        assert_eq!(why_not_listed(service_called("ollama").unwrap(), not_running), "Ollama is not running on this computer.");
        assert_eq!(why_not_listed(service_called("openai").unwrap(), down), "The AI service could not be reached: no route");
    }
}
