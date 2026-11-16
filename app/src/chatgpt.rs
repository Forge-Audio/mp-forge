//! The musician's ChatGPT account as the app keeps it, for using their own ChatGPT plan.
//!
//! Everything is kept in this computer's keychain and nowhere else. While the musician is signed in, the
//! whole account is kept under one name. Signed out, only its registration with OpenAI is kept, under
//! another, so that whether they are signed in can be told without reading anything.
//!
//! OpenAI replaces the renewing token every time it is used, so only one renewal runs at a time and what
//! comes back is kept before it is used.

use std::sync::Mutex;

use workbench_engine::agent::ModelError;
use workbench_engine::browser_sign_in::Patience;
use workbench_engine::chatgpt_plan::{self, Account, OpenAiPages, RenewalProblem, SIGN_IN_AGAIN, SignInMeans, seconds_now};
use workbench_engine::plan_model::PlanAccess;
use workbench_engine::providers::OfferedModel;

use crate::key_store;

/// The word the screen knows the ChatGPT plan by, and the name its account is kept under while signed in.
pub const ACCOUNT: &str = "chatgpt";
const REGISTRATION: &str = "chatgpt-registration";
const NOT_SIGNED_IN: &str = "Sign in with ChatGPT first: pick one of its models from the menu at the top.";
const SIGNED_OUT_HERE_ONLY: &str =
    "You are signed out here, but OpenAI did not confirm it. To be sure, disconnect MP Forge in ChatGPT's settings, under the apps that use your plan.";

static ONE_RENEWAL_AT_A_TIME: Mutex<()> = Mutex::new(());

pub fn is_signed_in() -> bool {
    key_store::holds(ACCOUNT)
}

fn kept_under(name: &str) -> Option<Account> {
    key_store::kept(name).as_deref().and_then(Account::read_from)
}

/// The account to sign in to: the one registered here before, or one made new for this computer.
fn account_to_sign_in_to() -> Result<Account, String> {
    kept_under(ACCOUNT).or_else(|| kept_under(REGISTRATION)).map_or_else(Account::new_on_this_computer, Ok)
}

/// Signs the musician in on OpenAI's own page, opened by `open_in_browser`, and keeps the account.
/// The app's registration with OpenAI is kept the moment it is given, whatever happens after.
pub fn sign_in(open_in_browser: &dyn Fn(&str) -> Result<(), String>, patience: &Patience) -> Result<(), String> {
    let keep_registration = |registered: &Account| drop(key_store::keep_record(REGISTRATION, &registered.signed_out().written()));
    let means = SignInMeans { pages: &OpenAiPages::real(), open_in_browser, patience, keep_registration: &keep_registration };
    let account = chatgpt_plan::signed_in(&account_to_sign_in_to()?, &means)?;
    key_store::keep_record(ACCOUNT, &account.written())
}

/// Signs out: OpenAI is told, and nothing that opens anything is kept. The registration stays for next time.
pub fn sign_out() -> Result<(), String> {
    let Some(account) = kept_under(ACCOUNT) else { return Ok(()) };
    let (signed_out, confirmed) = chatgpt_plan::signed_out(&account, &OpenAiPages::real());
    keep_signed_out(&signed_out)?;
    if confirmed { Ok(()) } else { Err(SIGNED_OUT_HERE_ONLY.to_string()) }
}

fn keep_signed_out(account: &Account) -> Result<(), String> {
    key_store::keep_record(REGISTRATION, &account.written())?;
    key_store::forget(ACCOUNT)
}

/// Whether a new token is wanted whatever time the kept one has left.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Renewal {
    WhenNearItsEnd,
    Now,
}

fn access_token(renewal: Renewal) -> Result<String, ModelError> {
    let _one_at_a_time = ONE_RENEWAL_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let account = kept_under(ACCOUNT).ok_or_else(|| ModelError::NoCredential(NOT_SIGNED_IN.to_string()))?;
    let pages = OpenAiPages::real();
    let renewed = if renewal == Renewal::Now { chatgpt_plan::renewed(&account, &pages, seconds_now()) } else { chatgpt_plan::with_access(&account, &pages, seconds_now()) };
    match renewed {
        Ok(with_access) => kept_if_changed(&account, with_access),
        Err(RenewalProblem::NotNow(cause)) => Err(ModelError::Unreachable(cause)),
        Err(RenewalProblem::SignInAgain) => {
            let _ = keep_signed_out(&account.signed_out());
            Err(ModelError::NoCredential(SIGN_IN_AGAIN.to_string()))
        }
    }
}

fn kept_if_changed(before: &Account, now: Account) -> Result<String, ModelError> {
    if now != *before {
        key_store::keep_record(ACCOUNT, &now.written()).map_err(ModelError::Unreachable)?;
    }
    now.access_token.ok_or_else(|| ModelError::NoCredential(SIGN_IN_AGAIN.to_string()))
}

pub fn models_offered() -> Result<Vec<OfferedModel>, ModelError> {
    chatgpt_plan::models_offered(&access_token(Renewal::WhenNearItsEnd)?, &OpenAiPages::real())
}

/// Access to the plan for as long as a piece of work lasts, read from the keychain and renewed as needed.
pub struct KeptAccess;

impl PlanAccess for KeptAccess {
    fn token(&mut self) -> Result<String, ModelError> {
        access_token(Renewal::WhenNearItsEnd)
    }

    fn token_after_refusal(&mut self) -> Result<String, ModelError> {
        access_token(Renewal::Now)
    }
}
