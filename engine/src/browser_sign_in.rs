//! Signing in to a service on its own web page, in place of pasting a key.
//!
//! The musician approves MP Forge on the service's page, in their own browser. The page then sends the
//! browser back to this computer with a code that is good once, and the code is exchanged for a key.
//! A secret made for this one attempt is proved at the exchange, so a code overheard on its way back is
//! no use to anyone else. MP Forge never sees the musician's password for the service.
//!
//! The way back from the browser and the secrets are shared by every such sign-in. The rest of this file is
//! OpenRouter's, which signs in this way: https://openrouter.ai/docs/use-cases/oauth-pkce

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use ring::digest::{SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};

use crate::model_http::{self, HTTP_OK, rejection};
use crate::providers::Provider;

/// Where a service has the musician approve the app, and where the code they come back with becomes a key.
pub struct SignInPages {
    pub approve_at: &'static str,
    pub exchange_at: &'static str,
}

pub const OPENROUTER: SignInPages = SignInPages { approve_at: "https://openrouter.ai/auth", exchange_at: "https://openrouter.ai/api/v1/auth/keys" };

/// The pages of a service that signs in this way. OpenRouter is the only one so far.
pub fn pages_of(provider: &Provider) -> Option<&'static SignInPages> {
    provider.signs_in_with_browser.then_some(&OPENROUTER)
}

/// OpenRouter's codes are good for ten minutes, so there is no use waiting longer for one.
pub const LONGEST_WAIT: Duration = Duration::from_secs(10 * 60);
const BETWEEN_LOOKS: Duration = Duration::from_millis(100);
/// A browser sometimes opens a connection ahead of time and says nothing on it. It is not waited on for long,
/// so the connection that does carry the answer is reached quickly.
const BROWSER_SAYS_IT_WITHIN: Duration = Duration::from_secs(2);
const CAME_BACK_TO: &str = "/signed-in";
const PROOF_METHOD: &str = "S256";
const BYTES_IN_A_SECRET: usize = 32;
const GAVE_UP: &str = "Signing in was stopped before it finished.";
const TOOK_TOO_LONG: &str = "Nobody finished signing in within ten minutes. Try again when you are ready.";
/// Shown when the browser comes back. The sign-in is not finished yet, so the page does not say it is. The
/// one-time code is taken out of the address bar, so it is not left in the browser's history.
const SIGNED_IN_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>MP Forge</title>\
    <body style=\"font-family: sans-serif; text-align: center; padding-top: 20vh\">\
    <h1>Back to MP Forge</h1><p>MP Forge is finishing the sign-in. You can close this page.</p>\
    <script>history.replaceState(null, \"\", location.pathname)</script></body>";
const NOTHING_HERE_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>MP Forge</title><body>Nothing to see here.</body>";

/// How long to wait for the browser, and how to learn that the musician has stopped waiting.
pub struct Patience<'a> {
    pub limit: Duration,
    pub has_given_up: &'a dyn Fn() -> bool,
}

/// What the browser brought back in its web address: each field by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brought(Vec<(String, String)>);

impl Brought {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.0.iter().find(|(known, _)| known == name).map(|(_, value)| value.as_str())
    }
}

/// The way back from the browser: a door on this computer alone, on a port of its own, open for one sign-in.
pub struct WayBack {
    listener: TcpListener,
}

impl WayBack {
    pub fn open() -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).and_then(|listener| listener.set_nonblocking(true).map(|()| listener));
        listener.map(|listener| Self { listener }).map_err(|error| format!("This computer would not let MP Forge wait for the sign-in: {error}"))
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|address| address.port()).unwrap_or_default()
    }

    /// Waits for the browser to come back to `page` bringing something `is_awaited` accepts. Anything else
    /// that knocks, such as a page that was not asked for, is told there is nothing here and the wait goes on.
    pub fn what_the_browser_brings(&self, page: &str, is_awaited: &dyn Fn(&Brought) -> bool, patience: &Patience) -> Result<Brought, String> {
        let started = Instant::now();
        loop {
            if (patience.has_given_up)() {
                return Err(GAVE_UP.to_string());
            }
            if started.elapsed() > patience.limit {
                return Err(TOOK_TOO_LONG.to_string());
            }
            match self.listener.accept() {
                Ok((browser, _)) => {
                    if let Some(brought) = answered(browser, page, is_awaited) {
                        return Ok(brought);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(BETWEEN_LOOKS),
                Err(error) => return Err(format!("The sign-in could not be heard back from: {error}")),
            }
        }
    }
}

/// Reads what the browser asked for and answers it. Only what was being waited for is handed on.
fn answered(mut browser: TcpStream, page: &str, is_awaited: &dyn Fn(&Brought) -> bool) -> Option<Brought> {
    let brought = brought_in(&first_line_from(&browser)?, page).filter(is_awaited);
    let shown = if brought.is_some() { SIGNED_IN_PAGE } else { NOTHING_HERE_PAGE };
    let _ = write!(browser, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{shown}", shown.len());
    brought
}

fn first_line_from(browser: &TcpStream) -> Option<String> {
    browser.set_nonblocking(false).ok()?;
    browser.set_read_timeout(Some(BROWSER_SAYS_IT_WITHIN)).ok()?;
    let mut line = String::new();
    BufReader::new(browser).read_line(&mut line).ok()?;
    Some(line)
}

/// The fields in a request such as `GET /signed-in?code=abc&state=xyz HTTP/1.1`, when it is for `page`.
fn brought_in(request_line: &str, page: &str) -> Option<Brought> {
    let address = request_line.split_whitespace().nth(1)?;
    let (asked_for, query) = address.split_once('?')?;
    let fields = query.split('&').filter_map(|pair| pair.split_once('=')).map(|(name, value)| (unescaped(name), unescaped(value)));
    (asked_for == page).then(|| Brought(fields.collect()))
}

/// One try at signing in to OpenRouter.
pub struct Attempt {
    way_back: WayBack,
    /// Known only to this attempt. The service is shown its fingerprint first and the secret itself at the exchange.
    secret: String,
    /// Sent out with the browser and expected back, so a page that was not asked for cannot answer for it.
    token: String,
}

impl Attempt {
    pub fn begin() -> Result<Self, String> {
        Ok(Self { way_back: WayBack::open()?, secret: random_words()?, token: random_words()? })
    }

    /// The page to open in the musician's browser.
    pub fn page_to_open(&self, pages: &SignInPages) -> String {
        let comes_back_to = format!("http://localhost:{}{CAME_BACK_TO}", self.way_back.port());
        let fields = [("callback_url", comes_back_to.as_str()), ("code_challenge", &fingerprint_of(&self.secret)), ("code_challenge_method", PROOF_METHOD), ("state", &self.token)];
        format!("{}?{}", pages.approve_at, as_a_query(&fields))
    }

    /// Waits for the musician to approve the app in their browser, then exchanges the code for the key.
    pub fn key_once_approved(&self, pages: &SignInPages, patience: &Patience) -> Result<String, String> {
        let is_for_this_attempt = |brought: &Brought| brought.field("state") == Some(&self.token) && brought.field("code").is_some_and(|code| !code.is_empty());
        let brought = self.way_back.what_the_browser_brings(CAME_BACK_TO, &is_for_this_attempt, patience)?;
        self.key_for(brought.field("code").unwrap_or_default(), pages)
    }

    fn key_for(&self, code: &str, pages: &SignInPages) -> Result<String, String> {
        let proof = json!({ "code": code, "code_verifier": self.secret, "code_challenge_method": PROOF_METHOD });
        let asked = model_http::quick_client().post(pages.exchange_at).header("content-type", "application/json").send(proof.to_string());
        let response = asked.map_err(|error| format!("The service could not be reached to finish signing in: {error}"))?;
        let status = response.status().as_u16();
        let reply = response.into_body().read_to_string().map_err(|error| format!("The service's answer could not be read: {error}"))?;
        if status != HTTP_OK {
            return Err(rejection(status, &reply).to_string());
        }
        let key = serde_json::from_str::<Value>(&reply).ok().and_then(|answer| answer["key"].as_str().map(str::to_string));
        key.filter(|key| !key.is_empty()).ok_or_else(|| "The service finished signing in but sent no key.".to_string())
    }
}

/// What a service is shown before the musician signs in: the secret's SHA-256, in the web's own base 64.
pub fn fingerprint_of(secret: &str) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(digest(&SHA256, secret.as_bytes()))
}

/// Words nobody could guess, made afresh each time: a secret, or a token to tell one attempt from another.
pub fn random_words() -> Result<String, String> {
    let mut bytes = [0u8; BYTES_IN_A_SECRET];
    SystemRandom::new().fill(&mut bytes).map_err(|_| "This computer could not make a secret for the sign-in.".to_string())?;
    Ok(BASE64_URL_SAFE_NO_PAD.encode(bytes))
}

/// Fields written the way a web address or a form carries them: `name=value&name=value`.
pub fn as_a_query(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(name, value)| format!("{name}={}", escaped(value))).collect::<Vec<String>>().join("&")
}

fn needs_no_escaping(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~".contains(&byte)
}

/// Text made safe to put in a web address.
fn escaped(text: &str) -> String {
    text.bytes().map(|byte| if needs_no_escaping(byte) { char::from(byte).to_string() } else { format!("%{byte:02X}") }).collect()
}

/// Text from a web address, read back as it was meant.
fn unescaped(text: &str) -> String {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&first, after)) = rest.split_first() {
        let escaped_byte = (first == b'%').then(|| after.get(..2)).flatten().and_then(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok());
        bytes.push(escaped_byte.unwrap_or(if first == b'+' { b' ' } else { first }));
        rest = if escaped_byte.is_some() { &after[2..] } else { after };
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    const NEVER: fn() -> bool = || false;

    fn patient_for(limit: Duration) -> Patience<'static> {
        Patience { limit, has_given_up: &NEVER }
    }

    /// Stands in for a service's exchange: answers one request with `reply`, and hands back what it was sent.
    fn exchange_answering(reply: &'static str) -> (String, thread::JoinHandle<String>) {
        let service = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = format!("http://127.0.0.1:{}/keys", service.local_addr().unwrap().port());
        let sent = thread::spawn(move || {
            let (mut asker, _) = service.accept().unwrap();
            let sent = whole_request_from(&mut asker);
            write!(asker, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).unwrap();
            sent
        });
        (address, sent)
    }

    /// Reads until the JSON that was sent has closed, however many pieces it arrives in.
    fn whole_request_from(asker: &mut TcpStream) -> String {
        let mut sent = String::new();
        let mut piece = [0u8; 1024];
        while !sent.ends_with('}') {
            let read = asker.read(&mut piece).unwrap();
            sent.push_str(&String::from_utf8_lossy(&piece[..read]));
        }
        sent
    }

    fn browser_comes_back_with(attempt: &Attempt, query: &str) {
        let port = attempt.way_back.port();
        let mut browser = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(browser, "GET {CAME_BACK_TO}?{query} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        let mut page = String::new();
        let _ = browser.read_to_string(&mut page);
    }

    #[test]
    fn the_fingerprint_is_the_one_the_standard_gives_for_its_own_example() {
        assert_eq!(fingerprint_of("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn the_page_to_open_carries_the_fingerprint_and_never_the_secret() {
        let attempt = Attempt::begin().unwrap();

        let page = attempt.page_to_open(&OPENROUTER);

        assert!(page.starts_with("https://openrouter.ai/auth?callback_url=http%3A%2F%2Flocalhost%3A"), "{page}");
        assert!(page.contains(&format!("code_challenge={}", fingerprint_of(&attempt.secret))) && page.contains("code_challenge_method=S256"));
        assert!(page.contains(&format!("state={}", attempt.token)) && !page.contains(&attempt.secret));
    }

    #[test]
    fn every_attempt_has_a_secret_and_a_token_of_its_own() {
        let (one, other) = (Attempt::begin().unwrap(), Attempt::begin().unwrap());

        assert_ne!(one.secret, other.secret);
        assert_ne!(one.secret, one.token);
        assert_eq!(one.secret.len(), 43);
    }

    #[test]
    fn what_the_browser_brings_is_read_from_the_page_that_was_asked_for_and_no_other() {
        let brought = brought_in("GET /signed-in?code=abc%2D1&state=tok HTTP/1.1", CAME_BACK_TO).unwrap();

        assert_eq!((brought.field("code"), brought.field("state"), brought.field("nothing")), (Some("abc-1"), Some("tok"), None));
        assert_eq!(brought_in("GET /elsewhere?code=abc&state=tok HTTP/1.1", CAME_BACK_TO), None);
        assert_eq!(brought_in("GET /favicon.ico HTTP/1.1", CAME_BACK_TO), None);
    }

    #[test]
    fn the_code_the_browser_brings_is_exchanged_for_the_key_with_the_secret_as_proof() {
        let attempt = Attempt::begin().unwrap();
        let (exchange_at, sent) = exchange_answering(r#"{"key":"key-from-the-service"}"#);
        let pages = SignInPages { approve_at: "http://127.0.0.1:9/auth", exchange_at: Box::leak(exchange_at.into_boxed_str()) };
        let query = format!("code=one-time-code&state={}", attempt.token);

        let key = thread::scope(|scope| {
            scope.spawn(|| {
                browser_comes_back_with(&attempt, "code=a-strangers-code&state=not-this-attempt");
                browser_comes_back_with(&attempt, &query);
            });
            attempt.key_once_approved(&pages, &patient_for(Duration::from_secs(10)))
        });

        assert_eq!(key, Ok("key-from-the-service".to_string()));
        let sent = sent.join().unwrap();
        assert!(sent.contains(r#""code":"one-time-code""#) && sent.contains(&attempt.secret) && sent.contains(r#""code_challenge_method":"S256""#), "{sent}");
    }

    #[test]
    fn waiting_ends_when_the_musician_gives_up_or_it_takes_too_long() {
        let attempt = Attempt::begin().unwrap();
        let given_up = AtomicBool::new(true);

        let stopped = attempt.key_once_approved(&OPENROUTER, &Patience { limit: Duration::from_secs(10), has_given_up: &|| given_up.load(Ordering::Relaxed) });
        let too_slow = attempt.key_once_approved(&OPENROUTER, &patient_for(Duration::ZERO));

        assert_eq!(stopped, Err(GAVE_UP.to_string()));
        assert_eq!(too_slow, Err(TOOK_TOO_LONG.to_string()));
    }
}
