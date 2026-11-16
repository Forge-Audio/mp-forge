//! The musician's own ChatGPT plan, used through OpenAI's "Sign in with ChatGPT".
//!
//! OpenAI lets a free, open-source app that runs on the user's computer do this without asking first:
//! https://developers.openai.com/siwc/token-sharing-open-source
//!
//! The musician signs in on OpenAI's page in their own browser and allows MP Forge to use their plan. The
//! browser comes back to this computer with a code that is good once, which is exchanged for tokens.
//! MP Forge never sees the musician's password. Who signed in is read from a token OpenAI signs, and the
//! signature is checked against OpenAI's published keys before anything is believed.
//!
//! This module holds what is kept of the account, signs in and out, and renews the tokens. Asking a model
//! for its next turn is in `plan_model`.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{RSA_PKCS1_2048_8192_SHA256, RsaPublicKeyComponents};
use serde_json::{Value, json};

use crate::agent::ModelError;
use crate::browser_sign_in::{Brought, Patience, WayBack, as_a_query, fingerprint_of, random_words};
use crate::model_http::{self, HTTP_OK, rejection};
use crate::providers::OfferedModel;

/// The name the musician sees on OpenAI's page. OpenAI asks that it is the app's own, the same everywhere.
const APP_NAME: &str = "MP Forge";
const ISSUER: &str = "https://auth.openai.com";
/// What the tokens are for: OpenAI's API.
const RESOURCE: &str = "https://api.openai.com/v1";
/// Sent in place of a client's name the first time, when OpenAI registers this app for the account.
const FIRST_TIME_CLIENT: &str = "dynamic_agent_client";
/// OpenAI wants exactly this page on 127.0.0.1. Only the port may differ from one sign-in to the next.
const CAME_BACK_TO: &str = "/auth/callback";
const SCOPES: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const MAY_USE_THE_PLAN: &str = "chatgpt.tokens.use.direct";
const SIGNATURE_KIND: &str = "RS256";
const FORM: &str = "application/x-www-form-urlencoded";
/// A token this close to running out is renewed before it is used.
const RENEWED_THIS_LONG_BEFORE_IT_ENDS: u64 = 5 * 60;
/// What OpenAI says of a renewing token that can no longer be used. Signing in again is the only way on.
const WORN_OUT: [&str; 6] = ["invalid_grant", "invalid_refresh_token", "token_expired", "refresh_token_expired", "refresh_token_invalidated", "refresh_token_reused"];

const DECLINED: &str = "MP Forge was not allowed to use your ChatGPT plan, so it is not connected. Nothing was changed.";
const PLAN_NOT_ALLOWED: &str = "You signed in, but MP Forge was not allowed to use your ChatGPT plan. Sign in again and allow it, or pick another service.";
const NOT_THE_SAME_ACCOUNT: &str = "That is a different ChatGPT account from the one MP Forge was connected to. Sign out here first to connect another.";
const CODE_REFUSED_TWICE: &str = "OpenAI would not finish the sign-in, twice over. MP Forge is registered with your account now, so trying again in a moment only needs you to pick the account.";
pub const SIGN_IN_AGAIN: &str = "Your ChatGPT sign-in has run out. Sign in again: pick a ChatGPT model from the menu at the top.";

/// Where OpenAI's sign-in and its models live. Tests point these at a stand-in.
#[derive(Debug, Clone)]
pub struct OpenAiPages {
    pub approve_at: String,
    pub tokens_at: String,
    pub sign_out_at: String,
    pub signing_keys_at: String,
    pub models_at: String,
}

impl OpenAiPages {
    pub fn real() -> Self {
        Self {
            approve_at: format!("{ISSUER}/api/accounts/authorize"),
            tokens_at: format!("{ISSUER}/api/accounts/oauth/token"),
            sign_out_at: format!("{ISSUER}/api/accounts/oauth/revoke"),
            signing_keys_at: format!("{ISSUER}/.well-known/jwks.json"),
            models_at: format!("{RESOURCE}/models"),
        }
    }
}

/// What is kept of the musician's ChatGPT account on this computer, in the keychain and nowhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Names this computer to OpenAI. Made once, it says nothing about who the musician is.
    pub host: String,
    /// The name OpenAI gave this app for this account, once it has registered.
    pub client: Option<String>,
    pub email: Option<String>,
    /// OpenAI's own name for the person, from a token whose signature was checked.
    pub person: Option<String>,
    pub access_token: Option<String>,
    pub renewing_token: Option<String>,
    /// When the access token stops working, in seconds since 1970.
    pub access_ends_at: u64,
}

impl Account {
    /// An account nobody has signed in to yet, with a name for this computer made here and now.
    pub fn new_on_this_computer() -> Result<Self, String> {
        Ok(Self { host: new_host_name()?, client: None, email: None, person: None, access_token: None, renewing_token: None, access_ends_at: 0 })
    }

    pub fn is_signed_in(&self) -> bool {
        self.renewing_token.is_some()
    }

    fn has_access_at(&self, now: u64) -> bool {
        self.access_token.is_some() && now + RENEWED_THIS_LONG_BEFORE_IT_ENDS < self.access_ends_at
    }

    /// The same account with nothing left that opens anything. Its registration with OpenAI is kept, so
    /// signing in again does not register the app a second time.
    pub fn signed_out(&self) -> Self {
        Self { access_token: None, renewing_token: None, access_ends_at: 0, ..self.clone() }
    }

    pub fn written(&self) -> String {
        json!({
            "host": self.host, "client": self.client, "email": self.email, "person": self.person,
            "accessToken": self.access_token, "renewingToken": self.renewing_token, "accessEndsAt": self.access_ends_at,
        })
        .to_string()
    }

    pub fn read_from(written: &str) -> Option<Self> {
        let kept: Value = serde_json::from_str(written).ok()?;
        let words = |field: &str| kept[field].as_str().map(str::to_string);
        Some(Self {
            host: words("host")?,
            client: words("client"),
            email: words("email"),
            person: words("person"),
            access_token: words("accessToken"),
            renewing_token: words("renewingToken"),
            access_ends_at: kept["accessEndsAt"].as_u64().unwrap_or(0),
        })
    }
}

/// A name for this computer in the form OpenAI accepts: `urn:uuid:` and a random UUID.
fn new_host_name() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    SystemRandom::new().fill(&mut bytes).map_err(|_| "This computer could not make a name for itself.".to_string())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!("urn:uuid:{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

pub fn seconds_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Signing in
// ---------------------------------------------------------------------------

/// What is made afresh for one sign-in and has to match when the browser comes back.
struct Attempt {
    comes_back_to: String,
    /// Known only to this attempt. OpenAI is shown its fingerprint first and the secret itself at the exchange.
    secret: String,
    /// Sent out with the browser and expected back unchanged.
    token: String,
    /// Expected inside the signed token that says who signed in.
    once: String,
}

/// What signing in needs from the app around it.
pub struct SignInMeans<'a> {
    pub pages: &'a OpenAiPages,
    /// Handed OpenAI's page, to open in the musician's browser.
    pub open_in_browser: &'a dyn Fn(&str) -> Result<(), String>,
    pub patience: &'a Patience<'a>,
    /// Handed the account as soon as OpenAI has registered the app for it, before anything else can go
    /// wrong, so that a later sign-in never registers the app a second time.
    pub keep_registration: &'a dyn Fn(&Account),
}

/// Why one try at signing in did not work.
enum Trouble {
    /// OpenAI would not take the one-time code. The account is as OpenAI registered it, to try again with.
    CodeRefused(Account),
    Other(String),
}

/// Signs the musician in. Answers with the account as it is to be kept from now on.
///
/// OpenAI says the code from a first sign-in can be refused after the app has been registered, and that
/// the way on is to sign in again with the registration just given. That is done once, without asking.
pub fn signed_in(account: &Account, means: &SignInMeans) -> Result<Account, String> {
    let first_try = match signed_in_once(account, means) {
        Err(Trouble::CodeRefused(registered)) => signed_in_once(&registered, means),
        outcome => outcome,
    };
    first_try.map_err(|trouble| match trouble {
        Trouble::CodeRefused(_) => CODE_REFUSED_TWICE.to_string(),
        Trouble::Other(why) => why,
    })
}

fn signed_in_once(account: &Account, means: &SignInMeans) -> Result<Account, Trouble> {
    let way_back = WayBack::open().map_err(Trouble::Other)?;
    let attempt = Attempt::coming_back_to(way_back.port()).map_err(Trouble::Other)?;
    (means.open_in_browser)(&page_to_open(account, &attempt, means.pages)).map_err(Trouble::Other)?;
    let is_for_this_attempt = |brought: &Brought| brought.field("state") == Some(&attempt.token);
    let brought = way_back.what_the_browser_brings(CAME_BACK_TO, &is_for_this_attempt, means.patience).map_err(Trouble::Other)?;
    let client = client_named_by(&brought, account).map_err(Trouble::Other)?;
    let registered = Account { client: Some(client.clone()), ..account.clone() };
    (means.keep_registration)(&registered);
    let tokens = exchanged(code_in(&brought).map_err(Trouble::Other)?, &attempt, &registered, means.pages)?;
    let expected = Expected { client: &client, once: &attempt.once, now: seconds_now() };
    let who = signing_keys(means.pages).and_then(|keys| who_signed_in(&tokens, &keys, &expected)).map_err(Trouble::Other)?;
    account_signed_in(&registered, &tokens, &who).map_err(Trouble::Other)
}

impl Attempt {
    fn coming_back_to(port: u16) -> Result<Self, String> {
        Ok(Self { comes_back_to: format!("http://127.0.0.1:{port}{CAME_BACK_TO}"), secret: random_words()?, token: random_words()?, once: random_words()? })
    }
}

/// Exchanges the one-time code for tokens, proving this attempt's secret.
fn exchanged(code: &str, attempt: &Attempt, registered: &Account, pages: &OpenAiPages) -> Result<Value, Trouble> {
    let client = registered.client.as_deref().unwrap_or_default();
    let fields = [("grant_type", "authorization_code"), ("client_id", client), ("code", code), ("code_verifier", &attempt.secret), ("redirect_uri", &attempt.comes_back_to), ("resource", RESOURCE)];
    let (status, answer) = posted_form(&pages.tokens_at, &fields).map_err(Trouble::Other)?;
    match (status, answer["error"].as_str()) {
        (HTTP_OK, _) => Ok(answer),
        (_, Some("invalid_grant")) => Err(Trouble::CodeRefused(registered.clone())),
        _ => Err(Trouble::Other(format!("OpenAI would not finish signing in: {}", rejection(status, &answer.to_string())))),
    }
}

/// OpenAI's page, with everything it needs to know in its address. A first sign-in asks OpenAI to register
/// the app for this account. A later one names the registration and the account it was for. No token is
/// ever put in the address, where other programs on the computer could read it.
fn page_to_open(account: &Account, attempt: &Attempt, pages: &OpenAiPages) -> String {
    let fingerprint = fingerprint_of(&attempt.secret);
    let mut fields = vec![
        ("response_type", "code"),
        ("redirect_uri", attempt.comes_back_to.as_str()),
        ("scope", SCOPES),
        ("resource", RESOURCE),
        ("state", attempt.token.as_str()),
        ("nonce", attempt.once.as_str()),
        ("code_challenge_method", "S256"),
        ("code_challenge", fingerprint.as_str()),
        ("ext_agent_host_id", account.host.as_str()),
    ];
    match &account.client {
        Some(client) => fields.push(("client_id", client)),
        None => fields.extend([("client_id", FIRST_TIME_CLIENT), ("agent_name_hint", APP_NAME)]),
    }
    fields.extend(account.email.as_deref().map(|email| ("login_hint", email)));
    format!("{}?{}", pages.approve_at, as_a_query(&fields))
}

fn code_in(brought: &Brought) -> Result<&str, String> {
    match (brought.field("error"), brought.field("code")) {
        (Some("access_denied"), _) => Err(DECLINED.to_string()),
        (Some(error), _) => Err(format!("OpenAI could not sign you in: {error}")),
        (None, Some(code)) if !code.is_empty() => Ok(code),
        _ => Err("OpenAI sent the browser back without what was needed to sign in. Try again.".to_string()),
    }
}

/// The name OpenAI knows this app by for this account. A first sign-in is given one. A later sign-in keeps
/// the one it has, and is never talked into another.
fn client_named_by(brought: &Brought, account: &Account) -> Result<String, String> {
    let given = brought.field("client_id").filter(|client| !client.is_empty() && *client != FIRST_TIME_CLIENT);
    match (&account.client, given) {
        (Some(kept), Some(given)) if kept != given => Err("OpenAI answered for a different registration than the one MP Forge has. Nothing was changed.".to_string()),
        (Some(kept), _) => Ok(kept.clone()),
        (None, Some(given)) => Ok(given.to_string()),
        (None, None) if brought.field("error").is_some() => code_in(brought).map(str::to_string),
        (None, None) => Err("OpenAI did not finish registering MP Forge for your account. Try again.".to_string()),
    }
}

fn account_signed_in(registered: &Account, tokens: &Value, who: &Value) -> Result<Account, String> {
    let person = who["sub"].as_str().map(str::to_string);
    if registered.person.is_some() && registered.person != person {
        return Err(NOT_THE_SAME_ACCOUNT.to_string());
    }
    let email = who["email"].as_str().map(str::to_string).or_else(|| registered.email.clone());
    let signed_in = Account { email, person, ..registered.clone() };
    with_new_tokens(&signed_in, tokens, seconds_now()).ok_or_else(|| PLAN_NOT_ALLOWED.to_string())
}

/// The account with the tokens OpenAI just gave, when they allow the plan to be used and can be renewed.
fn with_new_tokens(account: &Account, tokens: &Value, now: u64) -> Option<Account> {
    let allowed = tokens["scope"].as_str().unwrap_or_default().split_whitespace().any(|scope| scope == MAY_USE_THE_PLAN);
    let words = |field: &str| tokens[field].as_str().filter(|token| !token.is_empty()).map(str::to_string);
    let (access_token, renewing_token) = (words("access_token")?, words("refresh_token")?);
    let access_ends_at = now + tokens["expires_in"].as_u64().unwrap_or(0);
    allowed.then(|| Account { access_token: Some(access_token), renewing_token: Some(renewing_token), access_ends_at, ..account.clone() })
}

// ---------------------------------------------------------------------------
// Renewing, and signing out
// ---------------------------------------------------------------------------

/// Why an account's tokens could not be renewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenewalProblem {
    /// The tokens are no use any more. The musician has to sign in again.
    SignInAgain,
    /// Something got in the way this time. What is kept is still good.
    NotNow(String),
}

/// The account with an access token that will work for a while yet. One that still has time left is handed
/// back as it is. OpenAI replaces the renewing token each time, so what comes back must be kept at once.
pub fn with_access(account: &Account, pages: &OpenAiPages, now: u64) -> Result<Account, RenewalProblem> {
    if account.has_access_at(now) { Ok(account.clone()) } else { renewed(account, pages, now) }
}

/// The account with new tokens, whatever time the old access token had left.
pub fn renewed(account: &Account, pages: &OpenAiPages, now: u64) -> Result<Account, RenewalProblem> {
    let (Some(client), Some(renewing_token)) = (&account.client, &account.renewing_token) else {
        return Err(RenewalProblem::SignInAgain);
    };
    let fields = [("grant_type", "refresh_token"), ("client_id", client.as_str()), ("refresh_token", renewing_token.as_str()), ("resource", RESOURCE)];
    let (status, answer) = posted_form(&pages.tokens_at, &fields).map_err(RenewalProblem::NotNow)?;
    if status != HTTP_OK {
        return Err(why_not_renewed(status, &answer));
    }
    let scope_kept = json!({ "scope": answer["scope"].as_str().unwrap_or(MAY_USE_THE_PLAN) });
    with_new_tokens(account, &with_field(&answer, "scope", &scope_kept["scope"]), now).ok_or(RenewalProblem::SignInAgain)
}

fn with_field(answer: &Value, field: &str, value: &Value) -> Value {
    let mut changed = answer.clone();
    changed[field] = value.clone();
    changed
}

fn why_not_renewed(status: u16, answer: &Value) -> RenewalProblem {
    let said = [&answer["error"], &answer["error"]["code"], &answer["code"]].into_iter().find_map(Value::as_str).unwrap_or_default();
    if WORN_OUT.contains(&said) { RenewalProblem::SignInAgain } else { RenewalProblem::NotNow(rejection(status, &answer.to_string()).to_string()) }
}

/// Tells OpenAI the renewing token is finished with. Says whether OpenAI confirmed it. Either way the
/// account handed back opens nothing any more.
pub fn signed_out(account: &Account, pages: &OpenAiPages) -> (Account, bool) {
    let confirmed = match (&account.client, &account.renewing_token) {
        (Some(client), Some(token)) => posted_form(&pages.sign_out_at, &[("token", token), ("token_type_hint", "refresh_token"), ("client_id", client)]).is_ok_and(|(status, _)| status == HTTP_OK),
        _ => true,
    };
    (account.signed_out(), confirmed)
}

// ---------------------------------------------------------------------------
// The models the account may use
// ---------------------------------------------------------------------------

/// The models the signed-in account may use with its plan, in the order OpenAI lists them.
pub fn models_offered(access_token: &str, pages: &OpenAiPages) -> Result<Vec<OfferedModel>, ModelError> {
    let asked = model_http::quick_client().get(&pages.models_at).header("authorization", format!("Bearer {access_token}")).call();
    let response = asked.map_err(|error| ModelError::Unreachable(error.to_string()))?;
    let status = response.status().as_u16();
    let reply = response.into_body().read_to_string().map_err(|error| ModelError::Unreachable(error.to_string()))?;
    if status != HTTP_OK {
        return Err(rejection(status, &reply));
    }
    let listing = serde_json::from_str::<Value>(&reply).map_err(|error| ModelError::Garbled(error.to_string()))?;
    Ok(models_in(&listing))
}

/// OpenAI marks the models meant for a list to pick from. Whether one can look at pictures is believed when
/// it says, and assumed when it does not.
fn models_in(listing: &Value) -> Vec<OfferedModel> {
    let listed = listing["models"].as_array().map(Vec::as_slice).unwrap_or_default();
    let sees_pictures = |model: &Value| model["input_modalities"].as_array().is_none_or(|taken_in| taken_in.iter().any(|kind| kind == "image"));
    let offered = |model: &Value| {
        let slug = model["slug"].as_str()?;
        let name = model["display_name"].as_str().unwrap_or(slug);
        Some(OfferedModel { id: slug.to_string(), name: name.to_string(), sees_pictures: sees_pictures(model), context_window: model["context_window"].as_u64(), efforts: Vec::new() })
    };
    listed.iter().filter(|model| model["visibility"] == "list").filter_map(offered).collect()
}

// ---------------------------------------------------------------------------
// Talking to OpenAI's sign-in, and checking what it signs
// ---------------------------------------------------------------------------

fn posted_form(address: &str, fields: &[(&str, &str)]) -> Result<(u16, Value), String> {
    let asked = model_http::quick_client().post(address).header("content-type", FORM).header("accept", "application/json").send(as_a_query(fields));
    let response = asked.map_err(|error| format!("OpenAI's sign-in could not be reached: {error}"))?;
    let status = response.status().as_u16();
    let reply = response.into_body().read_to_string().map_err(|error| format!("OpenAI's answer could not be read: {error}"))?;
    Ok((status, serde_json::from_str(&reply).unwrap_or(Value::String(reply))))
}

fn signing_keys(pages: &OpenAiPages) -> Result<Value, String> {
    let asked = model_http::quick_client().get(&pages.signing_keys_at).call().map_err(|error| format!("OpenAI's signing keys could not be fetched: {error}"))?;
    let reply = asked.into_body().read_to_string().map_err(|error| format!("OpenAI's signing keys could not be read: {error}"))?;
    serde_json::from_str(&reply).map_err(|error| format!("OpenAI's signing keys could not be read: {error}"))
}

/// What the token that says who signed in has to be for.
struct Expected<'a> {
    client: &'a str,
    once: &'a str,
    now: u64,
}

/// Who signed in, read from the token OpenAI signs. Nothing in it is believed until the signature has been
/// checked against OpenAI's keys and the token is seen to be for this app, this attempt and this hour.
fn who_signed_in(tokens: &Value, keys: &Value, expected: &Expected) -> Result<Value, String> {
    let token = tokens["id_token"].as_str().ok_or("OpenAI did not say who signed in.")?;
    let who = checked_claims(token, keys).ok_or("The token that says who signed in was not signed by OpenAI. Nothing was changed.")?;
    let is_for_this_app = who["aud"] == expected.client || who["aud"].as_array().is_some_and(|audiences| audiences.iter().any(|audience| audience == expected.client));
    let is_as_expected = who["iss"] == ISSUER && is_for_this_app && who["nonce"] == expected.once && who["exp"].as_u64().is_some_and(|ends| ends > expected.now);
    if is_as_expected && who["sub"].is_string() { Ok(who) } else { Err("The token that says who signed in was not made for this sign-in. Nothing was changed.".to_string()) }
}

/// The claims inside a signed token, when one of the keys given really signed it.
fn checked_claims(token: &str, keys: &Value) -> Option<Value> {
    let parts: Vec<&str> = token.split('.').collect();
    let [heading, claims, signature] = parts.as_slice() else { return None };
    let heading: Value = decoded_json(heading)?;
    let key = keys["keys"].as_array()?.iter().find(|key| key["kid"] == heading["kid"] && key["kty"] == "RSA")?;
    let key = RsaPublicKeyComponents { n: decoded(key["n"].as_str()?)?, e: decoded(key["e"].as_str()?)? };
    let signed = format!("{}.{claims}", parts[0]);
    let is_signed_by_it = heading["alg"] == SIGNATURE_KIND && key.verify(&RSA_PKCS1_2048_8192_SHA256, signed.as_bytes(), &decoded(signature)?).is_ok();
    is_signed_by_it.then(|| decoded_json(claims)).flatten()
}

fn decoded(web_base64: &str) -> Option<Vec<u8>> {
    BASE64_URL_SAFE_NO_PAD.decode(web_base64.trim_end_matches('=')).ok()
}

fn decoded_json(web_base64: &str) -> Option<Value> {
    serde_json::from_slice(&decoded(web_base64)?).ok()
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::io::Read;
    use std::net::TcpStream;

    /// Reads the heading and as much of the body as the heading says there is. A stand-in service that answers
    /// and hangs up with some of the request still unread is cut off by Windows before its answer arrives.
    pub(crate) fn whole_request_from(asker: &mut TcpStream) -> String {
        let mut request = String::new();
        let mut piece = [0u8; 2048];
        while !is_whole(&request) {
            let read = asker.read(&mut piece).unwrap_or(0);
            if read == 0 {
                break;
            }
            request.push_str(&String::from_utf8_lossy(&piece[..read]));
        }
        request
    }

    fn is_whole(request: &str) -> bool {
        request.split_once("\r\n\r\n").is_some_and(|(heading, body)| body.len() >= body_length_said_in(heading))
    }

    fn body_length_said_in(heading: &str) -> usize {
        let said_on = |line: &str| line.to_ascii_lowercase().strip_prefix("content-length:").and_then(|length| length.trim().parse().ok());
        heading.lines().find_map(said_on).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use base64::prelude::BASE64_STANDARD;
    use ring::rsa::KeyPair;
    use ring::signature::RSA_PKCS1_SHA256;

    use super::test_support::whole_request_from;
    use super::*;

    const MADE_UP_SIGNING_KEY: &str = include_str!("../tests/fixtures/made_up_signing_key.pkcs8.b64");
    const NEVER: fn() -> bool = || false;
    const PATIENT: Patience<'static> = Patience { limit: Duration::from_secs(10), has_given_up: &NEVER };

    fn made_up_key() -> KeyPair {
        KeyPair::from_pkcs8(&BASE64_STANDARD.decode(MADE_UP_SIGNING_KEY.trim()).unwrap()).unwrap()
    }

    /// The public half of the made-up key, published the way OpenAI publishes its own.
    fn published_keys() -> Value {
        let public: RsaPublicKeyComponents<Vec<u8>> = RsaPublicKeyComponents::from(made_up_key().public());
        json!({ "keys": [{ "kty": "RSA", "kid": "made-up", "alg": "RS256", "n": BASE64_URL_SAFE_NO_PAD.encode(public.n), "e": BASE64_URL_SAFE_NO_PAD.encode(public.e) }] })
    }

    /// A token signed with the made-up key, as OpenAI signs the one that says who signed in.
    fn signed_token(claims: &Value) -> String {
        let key = made_up_key();
        let heading = json!({ "alg": "RS256", "kid": "made-up", "typ": "JWT" });
        let signed = format!("{}.{}", BASE64_URL_SAFE_NO_PAD.encode(heading.to_string()), BASE64_URL_SAFE_NO_PAD.encode(claims.to_string()));
        let mut signature = vec![0u8; key.public().modulus_len()];
        key.sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), signed.as_bytes(), &mut signature).unwrap();
        format!("{signed}.{}", BASE64_URL_SAFE_NO_PAD.encode(signature))
    }

    fn claims_for(client: &str, once: &str) -> Value {
        json!({ "iss": ISSUER, "aud": client, "sub": "person-1", "email": "musician@example.com", "nonce": once, "exp": seconds_now() + 600 })
    }

    /// Stands in for OpenAI: answers each page asked for with what it was told to say, and remembers
    /// what it was sent.
    struct StandIn {
        address: String,
        /// What each page says, in turn. Its last answer is given again for as long as it is asked.
        answers: Arc<Mutex<HashMap<&'static str, Vec<(u16, String)>>>>,
        sent: Arc<Mutex<Vec<String>>>,
        /// Every page the musician's browser was sent to, and how many one-time codes are to be refused.
        pages_opened: Mutex<Vec<String>>,
        codes_to_refuse: usize,
        registrations_kept: Mutex<Vec<Account>>,
    }

    impl StandIn {
        fn start() -> Self {
            let service = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let address = format!("http://127.0.0.1:{}", service.local_addr().unwrap().port());
            let (answers, sent) = (Arc::new(Mutex::new(HashMap::new())), Arc::new(Mutex::new(Vec::new())));
            let (its_answers, its_record) = (Arc::clone(&answers), Arc::clone(&sent));
            thread::spawn(move || {
                for asker in service.incoming().flatten() {
                    answer(asker, &its_answers, &its_record);
                }
            });
            Self { address, answers, sent, pages_opened: Mutex::new(Vec::new()), codes_to_refuse: 0, registrations_kept: Mutex::new(Vec::new()) }
        }

        fn refusing_codes(self, codes_to_refuse: usize) -> Self {
            Self { codes_to_refuse, ..self }
        }

        /// Signs in against this stand-in, with a browser that comes back for `client` bringing `brings`.
        fn sign_in(&self, account: &Account, client: &'static str, brings: &'static str) -> Result<Account, String> {
            let (pages, browser) = (self.pages(), browser_that(self, client, brings));
            let keep_registration = |registered: &Account| self.registrations_kept.lock().unwrap().push(registered.clone());
            signed_in(account, &SignInMeans { pages: &pages, open_in_browser: &browser, patience: &PATIENT, keep_registration: &keep_registration })
        }

        fn says(&self, page: &'static str, answer: &Value) {
            self.answers.lock().unwrap().insert(page, vec![(200, answer.to_string())]);
        }

        /// The page refuses the next thing it is asked, and answers as before after that.
        fn refuses_once(&self, page: &'static str, refusal: &Value) {
            self.answers.lock().unwrap().entry(page).or_default().insert(0, (400, refusal.to_string()));
        }

        fn pages(&self) -> OpenAiPages {
            let at = |page: &str| format!("{}{page}", self.address);
            OpenAiPages { approve_at: at("/authorize"), tokens_at: at("/token"), sign_out_at: at("/revoke"), signing_keys_at: at("/keys"), models_at: at("/models") }
        }

        fn was_sent(&self, words: &str) -> bool {
            self.sent.lock().unwrap().iter().any(|request| request.contains(words))
        }
    }

    fn answer(mut asker: TcpStream, answers: &Mutex<HashMap<&'static str, Vec<(u16, String)>>>, sent: &Mutex<Vec<String>>) {
        let request = whole_request_from(&mut asker);
        let page = request.split_whitespace().nth(1).unwrap_or_default().to_string();
        let (status, body) = next_answer_of(answers.lock().unwrap().get_mut(page.as_str()));
        sent.lock().unwrap().push(request);
        let _ = write!(asker, "HTTP/1.1 {status} Said\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    }

    fn next_answer_of(in_turn: Option<&mut Vec<(u16, String)>>) -> (u16, String) {
        match in_turn {
            Some(answers) if answers.len() > 1 => answers.remove(0),
            Some(answers) => answers.first().cloned().unwrap_or((404, String::new())),
            None => (404, String::new()),
        }
    }

    fn field_in(page: &str, name: &str) -> String {
        let query = page.split_once('?').unwrap().1;
        let value = query.split('&').filter_map(|pair| pair.split_once('=')).find(|(known, _)| *known == name).map(|(_, value)| value.to_string());
        value.unwrap_or_default().replace("%3A", ":").replace("%2F", "/")
    }

    /// The musician's browser: reads OpenAI's page from its address, has the stand-in ready the tokens for
    /// this attempt, and comes back to this computer with `brings` and the attempt's own token.
    fn browser_that<'a>(stand_in: &'a StandIn, client: &'static str, brings: &'static str) -> impl Fn(&str) -> Result<(), String> + 'a {
        move |page: &str| {
            let who = signed_token(&claims_for(client, &field_in(page, "nonce")));
            stand_in.says("/token", &json!({ "access_token": "access-1", "refresh_token": "renewing-1", "id_token": who, "token_type": "Bearer", "expires_in": 3600, "scope": SCOPES }));
            stand_in.pages_opened.lock().unwrap().push(page.to_string());
            if stand_in.pages_opened.lock().unwrap().len() <= stand_in.codes_to_refuse {
                stand_in.refuses_once("/token", &json!({ "error": "invalid_grant" }));
            }
            let comes_back_to = format!("{}?{brings}&state={}", field_in(page, "redirect_uri"), field_in(page, "state"));
            thread::spawn(move || {
                let (address, asked_for) = comes_back_to.strip_prefix("http://").unwrap().split_once('/').unwrap();
                let mut browser = TcpStream::connect(address).unwrap();
                write!(browser, "GET /{asked_for} HTTP/1.1\r\nHost: {address}\r\n\r\n").unwrap();
                let _ = browser.read_to_string(&mut String::new());
            });
            Ok(())
        }
    }

    fn signed_in_account() -> Account {
        Account {
            host: "urn:uuid:11111111-1111-4111-8111-111111111111".to_string(),
            client: Some("oaiapp_made_up".to_string()),
            email: Some("musician@example.com".to_string()),
            person: Some("person-1".to_string()),
            access_token: Some("access-1".to_string()),
            renewing_token: Some("renewing-1".to_string()),
            access_ends_at: 10_000,
        }
    }

    #[test]
    fn this_computer_is_named_in_the_form_openai_accepts() {
        let (one, other) = (new_host_name().unwrap(), new_host_name().unwrap());

        assert!(one.starts_with("urn:uuid:") && one.len() == 45 && one.as_bytes()[23] == b'4', "{one}");
        assert_ne!(one, other);
    }

    #[test]
    fn what_is_kept_of_an_account_is_read_back_as_it_was_written() {
        let account = signed_in_account();
        let new = Account::new_on_this_computer().unwrap();

        assert_eq!(Account::read_from(&account.written()), Some(account));
        assert_eq!(Account::read_from(&new.written()), Some(new));
        assert_eq!(Account::read_from("not an account"), None);
    }

    #[test]
    fn signing_out_keeps_the_registration_and_nothing_that_opens_anything() {
        let signed_out = signed_in_account().signed_out();

        assert!(!signed_out.is_signed_in());
        assert_eq!((signed_out.access_token, signed_out.renewing_token), (None, None));
        assert_eq!((signed_out.client.as_deref(), signed_out.person.as_deref()), (Some("oaiapp_made_up"), Some("person-1")));
    }

    #[test]
    fn a_first_sign_in_asks_to_be_registered_and_a_later_one_names_its_registration() {
        let attempt = Attempt { comes_back_to: "http://127.0.0.1:1455/auth/callback".to_string(), secret: "secret".to_string(), token: "token".to_string(), once: "once".to_string() };
        let pages = OpenAiPages::real();

        let first = page_to_open(&Account::new_on_this_computer().unwrap(), &attempt, &pages);
        let later = page_to_open(&signed_in_account(), &attempt, &pages);

        assert!(first.starts_with("https://auth.openai.com/api/accounts/authorize?response_type=code&redirect_uri=http%3A%2F%2F127.0.0.1%3A1455%2Fauth%2Fcallback"), "{first}");
        assert!(first.contains("client_id=dynamic_agent_client&agent_name_hint=MP%20Forge") && first.contains("ext_agent_host_id=urn%3Auuid%3A"));
        assert!(first.contains("scope=openid%20profile%20email%20offline_access%20resource.invoke%20chatgpt.tokens.use.direct") && first.contains("resource=https%3A%2F%2Fapi.openai.com%2Fv1"));
        assert!(first.contains(&format!("code_challenge={}", fingerprint_of("secret"))) && !first.contains("secret&"));
        assert!(later.contains("client_id=oaiapp_made_up") && !later.contains("agent_name_hint") && later.contains("login_hint=musician%40example.com"));
        assert!(!later.contains("access-1") && !later.contains("renewing-1"), "no token is ever put in a web address");
    }

    #[test]
    fn a_token_is_believed_only_when_openais_key_signed_it_for_this_sign_in() {
        let keys = published_keys();
        let expected = Expected { client: "oaiapp_made_up", once: "once", now: seconds_now() };
        let who_from = |claims: Value| who_signed_in(&json!({ "id_token": signed_token(&claims) }), &keys, &expected);
        let good = signed_token(&claims_for("oaiapp_made_up", "once"));
        let (heading_and_claims, signature) = good.rsplit_once('.').unwrap();
        let forged_claims = BASE64_URL_SAFE_NO_PAD.encode(claims_for("oaiapp_made_up", "once").to_string().replace("person-1", "somebody-else"));
        let forged = format!("{}.{forged_claims}.{signature}", heading_and_claims.split_once('.').unwrap().0);

        assert_eq!(who_from(claims_for("oaiapp_made_up", "once")).unwrap()["sub"], "person-1");
        assert!(who_from(claims_for("another-app", "once")).is_err());
        assert!(who_from(claims_for("oaiapp_made_up", "another-attempt")).is_err());
        assert!(who_from(json!({ "iss": ISSUER, "aud": "oaiapp_made_up", "sub": "person-1", "nonce": "once", "exp": 5 })).is_err());
        assert!(who_signed_in(&json!({ "id_token": forged }), &keys, &expected).is_err());
        assert!(who_signed_in(&json!({ "id_token": good }), &json!({ "keys": [] }), &expected).is_err());
    }

    #[test]
    fn a_first_sign_in_registers_the_app_and_keeps_who_signed_in_with_their_tokens() {
        let stand_in = StandIn::start();
        stand_in.says("/keys", &published_keys());
        let new = Account::new_on_this_computer().unwrap();

        let account = stand_in.sign_in(&new, "oaiapp_made_up", "code=one-time-code&client_id=oaiapp_made_up").unwrap();

        assert_eq!((account.client.as_deref(), account.person.as_deref(), account.email.as_deref()), (Some("oaiapp_made_up"), Some("person-1"), Some("musician@example.com")));
        assert_eq!((account.access_token.as_deref(), account.renewing_token.as_deref(), account.host.as_str()), (Some("access-1"), Some("renewing-1"), new.host.as_str()));
        assert!(account.is_signed_in() && account.access_ends_at > seconds_now() + 3000);
        assert!(stand_in.was_sent("grant_type=authorization_code&client_id=oaiapp_made_up&code=one-time-code&code_verifier="));
        assert!(stand_in.was_sent("redirect_uri=http%3A%2F%2F127.0.0.1%3A") && stand_in.was_sent("resource=https%3A%2F%2Fapi.openai.com%2Fv1"));
    }

    #[test]
    fn a_first_code_openai_refuses_is_followed_by_signing_in_again_with_the_registration_just_given() {
        let stand_in = StandIn::start().refusing_codes(1);
        stand_in.says("/keys", &published_keys());
        let new = Account::new_on_this_computer().unwrap();

        let account = stand_in.sign_in(&new, "oaiapp_made_up", "code=one-time-code&client_id=oaiapp_made_up").unwrap();

        let pages_opened = stand_in.pages_opened.lock().unwrap();
        assert!(account.is_signed_in() && account.client.as_deref() == Some("oaiapp_made_up"));
        assert!(pages_opened[0].contains("client_id=dynamic_agent_client") && pages_opened[0].contains("agent_name_hint"));
        assert!(pages_opened[1].contains("client_id=oaiapp_made_up") && !pages_opened[1].contains("agent_name_hint"), "{}", pages_opened[1]);
        assert_eq!(pages_opened.len(), 2);
        assert_eq!(stand_in.registrations_kept.lock().unwrap()[0].client.as_deref(), Some("oaiapp_made_up"));
    }

    #[test]
    fn a_code_refused_twice_stops_there_with_the_registration_kept_for_next_time() {
        let stand_in = StandIn::start().refusing_codes(2);
        stand_in.says("/keys", &published_keys());

        let refused = stand_in.sign_in(&Account::new_on_this_computer().unwrap(), "oaiapp_made_up", "code=one-time-code&client_id=oaiapp_made_up");

        assert_eq!(refused, Err(CODE_REFUSED_TWICE.to_string()));
        assert_eq!(stand_in.pages_opened.lock().unwrap().len(), 2);
        assert!(stand_in.registrations_kept.lock().unwrap().iter().all(|kept| kept.client.as_deref() == Some("oaiapp_made_up") && !kept.is_signed_in()));
    }

    #[test]
    fn a_musician_who_declines_or_another_account_changes_nothing() {
        let stand_in = StandIn::start();
        stand_in.says("/keys", &published_keys());
        let new = Account::new_on_this_computer().unwrap();
        let of_somebody_else = Account { person: Some("person-2".to_string()), ..signed_in_account() };

        let declined = stand_in.sign_in(&new, "oaiapp_made_up", "error=access_denied");
        let unregistered = stand_in.sign_in(&new, "oaiapp_made_up", "code=one-time-code");
        let another_person = stand_in.sign_in(&of_somebody_else, "oaiapp_made_up", "code=one-time-code");
        let another_registration = stand_in.sign_in(&signed_in_account(), "oaiapp_made_up", "code=one-time-code&client_id=oaiapp_other");

        assert_eq!(declined, Err(DECLINED.to_string()));
        assert!(unregistered.unwrap_err().contains("did not finish registering"));
        assert_eq!(another_person, Err(NOT_THE_SAME_ACCOUNT.to_string()));
        assert!(another_registration.unwrap_err().contains("different registration"));
    }

    #[test]
    fn signing_in_without_allowing_the_plan_is_not_signed_in() {
        let tokens = json!({ "access_token": "a", "refresh_token": "r", "expires_in": 3600, "scope": "openid profile email offline_access" });

        assert_eq!(with_new_tokens(&signed_in_account(), &tokens, 100), None);
    }

    #[test]
    fn an_access_token_with_time_left_is_used_as_it_is_and_one_near_its_end_is_renewed() {
        let stand_in = StandIn::start();
        stand_in.says("/token", &json!({ "access_token": "access-2", "refresh_token": "renewing-2", "expires_in": 3600, "scope": SCOPES }));
        let account = signed_in_account();

        let still_good = with_access(&account, &stand_in.pages(), 9_000).unwrap();
        let renewed = with_access(&account, &stand_in.pages(), 9_900).unwrap();

        assert_eq!(still_good, account);
        assert_eq!((renewed.access_token.as_deref(), renewed.renewing_token.as_deref(), renewed.access_ends_at), (Some("access-2"), Some("renewing-2"), 13_500));
        assert_eq!((renewed.client, renewed.person), (account.client, account.person));
        assert!(stand_in.was_sent("grant_type=refresh_token&client_id=oaiapp_made_up&refresh_token=renewing-1&resource="));
    }

    #[test]
    fn a_worn_out_renewing_token_means_signing_in_again_and_any_other_trouble_does_not() {
        let worn_out = why_not_renewed(400, &json!({ "error": "invalid_grant" }));
        let reused = why_not_renewed(400, &json!({ "error": { "code": "refresh_token_reused" } }));
        let busy = why_not_renewed(503, &json!({ "error": "temporarily_unavailable" }));
        let unreachable = renewed(&signed_in_account(), &OpenAiPages { tokens_at: "http://127.0.0.1:9/token".to_string(), ..OpenAiPages::real() }, 0);

        assert_eq!((worn_out, reused), (RenewalProblem::SignInAgain, RenewalProblem::SignInAgain));
        assert!(matches!(busy, RenewalProblem::NotNow(_)) && matches!(unreachable, Err(RenewalProblem::NotNow(_))));
        assert_eq!(renewed(&signed_in_account().signed_out(), &OpenAiPages::real(), 0), Err(RenewalProblem::SignInAgain));
    }

    #[test]
    fn signing_out_tells_openai_and_says_whether_it_was_confirmed() {
        let stand_in = StandIn::start();
        stand_in.says("/revoke", &json!({}));
        let nobody_listens = OpenAiPages { sign_out_at: "http://127.0.0.1:9/revoke".to_string(), ..OpenAiPages::real() };

        let (signed_out_account, confirmed) = signed_out(&signed_in_account(), &stand_in.pages());
        let (still_signed_out, confirmed_elsewhere) = signed_out(&signed_in_account(), &nobody_listens);

        assert!(confirmed && !signed_out_account.is_signed_in());
        assert!(!confirmed_elsewhere && !still_signed_out.is_signed_in());
        assert!(stand_in.was_sent("token=renewing-1&token_type_hint=refresh_token&client_id=oaiapp_made_up"));
    }

    #[test]
    fn only_the_models_meant_for_a_list_are_offered_by_the_names_openai_shows() {
        let stand_in = StandIn::start();
        stand_in.says("/models", &json!({ "models": [
            { "slug": "made-up-large", "display_name": "Made-Up Large", "visibility": "list", "context_window": 400_000 },
            { "slug": "made-up-hidden", "display_name": "Hidden", "visibility": "hide" },
            { "slug": "made-up-words-only", "visibility": "list", "input_modalities": ["text"] },
        ] }));

        let offered = models_offered("access-1", &stand_in.pages()).unwrap();

        assert_eq!(offered[0], OfferedModel { id: "made-up-large".to_string(), name: "Made-Up Large".to_string(), sees_pictures: true, context_window: Some(400_000), efforts: Vec::new() });
        assert_eq!((offered.len(), offered[1].name.as_str(), offered[1].sees_pictures), (2, "made-up-words-only", false));
        assert!(stand_in.was_sent("authorization: Bearer access-1") || stand_in.was_sent("Authorization: Bearer access-1"));
    }
}
