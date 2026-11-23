//! What every AI connection over the internet has in common: one way to ask, to wait, to try again,
//! and to say why a service said no.

use std::io::{BufRead, BufReader};
use std::thread;
use std::time::Duration;

use serde_json::Value;
use ureq::Body;
use ureq::http::{HeaderMap, Response};

use crate::agent::ModelError;

const REQUEST_TIME_LIMIT: Duration = Duration::from_secs(20 * 60);
const WAITS_BEFORE_RETRYING: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(8)];
/// A question with a short answer, such as which models a service offers, is not waited on for long.
const QUICK_TIME_LIMIT: Duration = Duration::from_secs(30);
/// Long enough to fetch a few hundred megabytes over an ordinary line, and no longer.
const DOWNLOAD_TIME_LIMIT: Duration = Duration::from_secs(10 * 60);
pub const HTTP_OK: u16 = 200;
const HTTP_TOO_MANY_REQUESTS: u16 = 429;
/// Where a service says for how many seconds it asks to be left alone.
const WAIT_ASKED_HEADER: &str = "retry-after";
/// The words of a service behind the one that was asked are shown up to this length.
const MOST_LETTERS_OF_ANOTHER_SERVICES_WORDS: usize = 300;

/// A client that hands back a refusal as a reply to read, and gives a long turn time to finish.
pub fn client() -> ureq::Agent {
    client_waiting(REQUEST_TIME_LIMIT)
}

pub fn quick_client() -> ureq::Agent {
    client_waiting(QUICK_TIME_LIMIT)
}

pub fn download_client() -> ureq::Agent {
    client_waiting(DOWNLOAD_TIME_LIMIT)
}

fn client_waiting(limit: Duration) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(limit)).build().into()
}

/// Asks, and asks again after a short wait when the service was busy or could not be reached.
pub fn with_retries<T>(ask: impl Fn() -> Result<T, ModelError>) -> Result<T, ModelError> {
    with_retries_while(ask, is_worth_retrying)
}

/// Asks, and asks again after a short wait for as long as `is_worth_another_try` says so of the refusal.
pub fn with_retries_while<T>(ask: impl Fn() -> Result<T, ModelError>, is_worth_another_try: impl Fn(&ModelError) -> bool) -> Result<T, ModelError> {
    let mut waits = WAITS_BEFORE_RETRYING.iter();
    loop {
        match ask() {
            Err(error) if is_worth_another_try(&error) => match waits.next() {
                Some(wait) => thread::sleep(*wait),
                None => return Err(error),
            },
            outcome => return outcome,
        }
    }
}

/// A service that is asked too often and says how long to wait is not asked again this soon.
pub fn is_worth_retrying(error: &ModelError) -> bool {
    match error {
        ModelError::Rejected { status, .. } => *status >= 500,
        ModelError::AskedTooOften { asks_to_wait, .. } => asks_to_wait.is_none(),
        ModelError::Unreachable(_) => true,
        ModelError::NoCredential(_) | ModelError::Garbled(_) => false,
    }
}

/// The events a service streams back when it says yes. Otherwise its refusal, which `in_words` makes of the
/// status and the text of the reply, with how long the service asks to be left alone when it says so.
pub fn event_stream_of(reply: Response<Body>, in_words: impl Fn(u16, &str) -> ModelError) -> Result<Box<dyn BufRead>, ModelError> {
    let status = reply.status().as_u16();
    if status == HTTP_OK {
        return Ok(Box::new(BufReader::new(reply.into_body().into_reader())));
    }
    let wait_asked = wait_asked_in(reply.headers());
    let said = reply.into_body().read_to_string().unwrap_or_default();
    Err(asking_to_wait(in_words(status, &said), wait_asked))
}

/// A wait is asked for as a whole number of seconds. Asked for as a date, it is not read.
fn wait_asked_in(headers: &HeaderMap) -> Option<Duration> {
    let seconds = headers.get(WAIT_ASKED_HEADER)?.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(seconds))
}

fn asking_to_wait(refusal: ModelError, wait_asked: Option<Duration>) -> ModelError {
    match refusal {
        ModelError::AskedTooOften { message, asks_to_wait } => ModelError::AskedTooOften { message, asks_to_wait: wait_asked.or(asks_to_wait) },
        other => other,
    }
}

/// A refusal, with the reason the service gave when it gave one in a form that can be read. A service that
/// says it is asked too often is told apart, since that passes by itself.
pub fn rejection(status: u16, reply: &str) -> ModelError {
    let stated_reason = serde_json::from_str::<Value>(reply).ok().and_then(|parsed| reason_in(&parsed));
    let message = stated_reason.unwrap_or_else(|| reply.trim().to_string());
    if status == HTTP_TOO_MANY_REQUESTS {
        return ModelError::AskedTooOften { message, asks_to_wait: None };
    }
    ModelError::Rejected { status, message }
}

/// Services word a refusal differently. These are the places the reason has been seen. A service that passes
/// requests on to others, as OpenRouter does, gives its own reason in general words ("Provider returned
/// error"), so what the service behind it said is put after them.
fn reason_in(reply: &Value) -> Option<String> {
    let places = [&reply["error"]["message"], &reply["error"], &reply["detail"], &reply["message"]];
    let stated = places.into_iter().find_map(|place| place.as_str().map(str::to_string))?;
    let behind_it = said_by_the_service_behind(&reply["error"]["metadata"]);
    Some(behind_it.map_or(stated.clone(), |behind_it| format!("{}. {behind_it}", stated.trim_end_matches('.'))))
}

/// OpenRouter names the service it passed the request to, hands on what that service answered, and gives a
/// short code for the kind of refusal. The answer is told when it can be read, and the code when it cannot.
fn said_by_the_service_behind(about_it: &Value) -> Option<String> {
    let code = || [&about_it["provider_code"], &about_it["error_type"]].into_iter().find_map(|code| code.as_str().map(str::to_string));
    let its_words = own_words_in(&about_it["raw"]).or_else(code)?;
    let its_name = about_it["provider_name"].as_str().unwrap_or("The service behind it");
    Some(format!("{its_name} said: {its_words}"))
}

/// What another service answered is handed on as it came: plain words, or a reply of its own, which may be
/// written out as text.
fn own_words_in(handed_on: &Value) -> Option<String> {
    let written_out = handed_on.as_str().and_then(|text| serde_json::from_str::<Value>(text).ok());
    let stated = reason_in(written_out.as_ref().unwrap_or(handed_on));
    let words = stated.or_else(|| handed_on.as_str().map(str::to_string))?;
    let words = words.trim();
    (!words.is_empty()).then(|| words.chars().take(MOST_LETTERS_OF_ANOTHER_SERVICES_WORDS).collect())
}

/// A stand-in for a real service, shared by the tests of every module that asks one.
#[cfg(test)]
pub mod test_support {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{Ipv4Addr, TcpListener, TcpStream};
    use std::thread;

    /// A service on this computer that answers one asker after another, each with the next of these
    /// answers, and is then gone. Returns its address.
    pub fn service_answering(answers: Vec<String>) -> String {
        let waiting = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = format!("http://{}", waiting.local_addr().unwrap());
        thread::spawn(move || {
            for answer in answers {
                let (asker, _) = waiting.accept().unwrap();
                answer_once_heard_out(asker, &answer);
            }
        });
        address
    }

    /// A yes that carries these words.
    pub fn yes_saying(words: &str) -> String {
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{words}", words.len())
    }

    /// The whole request is read before the answer is written. An asker that is hung up on while it still
    /// has something to send takes the line for broken, on some systems, and never reads the answer.
    fn answer_once_heard_out(asker: TcpStream, answer: &str) {
        let mut request = BufReader::new(asker);
        let length_of_what_follows = length_said_in_the_opening_of(&mut request);
        std::io::copy(&mut request.by_ref().take(length_of_what_follows), &mut std::io::sink()).unwrap();
        request.into_inner().write_all(answer.as_bytes()).unwrap();
    }

    fn length_said_in_the_opening_of(request: &mut impl BufRead) -> u64 {
        let mut length = 0;
        loop {
            let mut line = String::new();
            request.read_line(&mut line).unwrap();
            match line.trim().split_once(':') {
                Some((name, value)) if name.eq_ignore_ascii_case("content-length") => length = value.trim().parse().unwrap(),
                Some(_) => {}
                None if line.trim().is_empty() => return length,
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::service_answering;
    use super::*;

    #[test]
    fn a_rejection_reports_the_reason_however_the_service_worded_it() {
        let reason_of = |reply: &str| match rejection(401, reply) {
            ModelError::Rejected { message, .. } => message,
            other => panic!("expected a rejection, got {other:?}"),
        };

        assert_eq!(reason_of(r#"{"error":{"message":"invalid key"}}"#), "invalid key");
        assert_eq!(reason_of(r#"{"error":"No credentials presented."}"#), "No credentials presented.");
        assert_eq!(reason_of(r#"{"detail":"Invalid API Key"}"#), "Invalid API Key");
        assert_eq!(reason_of("overloaded"), "overloaded");
    }

    #[test]
    fn a_busy_service_is_worth_asking_again_and_a_refused_key_is_not() {
        assert!(is_worth_retrying(&rejection(529, "overloaded")));
        assert!(is_worth_retrying(&rejection(429, "slow down")));
        assert!(!is_worth_retrying(&rejection(401, "no")));
    }

    #[test]
    fn a_service_that_is_asked_too_often_is_told_apart_and_not_asked_again_at_once_when_it_says_how_long_to_wait() {
        let asked_too_often = rejection(429, r#"{"error":{"message":"slow down"}}"#);
        let saying_how_long = asking_to_wait(rejection(429, "slow down"), Some(Duration::from_secs(30)));

        assert!(matches!(&asked_too_often, ModelError::AskedTooOften { message, asks_to_wait: None } if message == "slow down"));
        assert!(matches!(&saying_how_long, ModelError::AskedTooOften { asks_to_wait: Some(wait), .. } if wait.as_secs() == 30));
        assert!(!is_worth_retrying(&saying_how_long));
        assert!(matches!(asking_to_wait(rejection(503, "busy"), Some(Duration::from_secs(30))), ModelError::Rejected { status: 503, .. }));
    }

    #[test]
    fn the_wait_a_service_asks_for_is_read_as_seconds_and_not_when_it_is_a_date() {
        let asking = |written: &'static str| {
            let mut headers = HeaderMap::new();
            headers.insert(WAIT_ASKED_HEADER, written.parse().unwrap());
            wait_asked_in(&headers)
        };

        assert_eq!(asking("45"), Some(Duration::from_secs(45)));
        assert_eq!(asking("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(wait_asked_in(&HeaderMap::new()), None);
    }

    #[test]
    fn a_reply_that_is_a_refusal_is_read_with_the_wait_its_service_asks_for_and_a_yes_as_its_events() {
        const TOO_OFTEN: &str = "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 17\r\nContent-Length: 33\r\nConnection: close\r\n\r\n{\"error\":{\"message\":\"slow down\"}}";
        const YES: &str = "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\ndata: hi\n";
        let asked = |answer: &str| event_stream_of(client().post(service_answering(vec![answer.to_string()])).send("{}").unwrap(), rejection);

        let refused = asked(TOO_OFTEN).err().unwrap();
        let events: Vec<String> = asked(YES).unwrap().lines().map(Result::unwrap).collect();

        assert!(matches!(&refused, ModelError::AskedTooOften { message, asks_to_wait: Some(wait) } if message == "slow down" && wait.as_secs() == 17), "{refused:?}");
        assert_eq!(events, ["data: hi"]);
    }

    #[test]
    fn what_the_service_behind_openrouter_said_is_put_after_openrouters_own_words() {
        let reason_of = |reply: &str| match rejection(429, reply) {
            ModelError::AskedTooOften { message, .. } => message,
            other => panic!("expected a service asked too often, got {other:?}"),
        };
        let with_its_reply_written_out = r#"{"error":{"code":429,"message":"Provider returned error","metadata":{"provider_name":"Google AI Studio","raw":"{\"error\":{\"code\":429,\"message\":\"Resource has been exhausted.\",\"status\":\"RESOURCE_EXHAUSTED\"}}"}}}"#;
        let with_plain_words = r#"{"error":{"message":"Provider returned error","metadata":{"provider_name":"Chutes","raw":"the model is temporarily rate-limited upstream"}}}"#;
        let with_a_code_only = r#"{"error":{"message":"Provider returned error","metadata":{"error_type":"rate_limit_exceeded","provider_code":"rate_limited"}}}"#;
        let with_nothing_more = r#"{"error":{"message":"Provider returned error","metadata":{}}}"#;

        assert_eq!(reason_of(with_its_reply_written_out), "Provider returned error. Google AI Studio said: Resource has been exhausted.");
        assert_eq!(reason_of(with_plain_words), "Provider returned error. Chutes said: the model is temporarily rate-limited upstream");
        assert_eq!(reason_of(with_a_code_only), "Provider returned error. The service behind it said: rate_limited");
        assert_eq!(reason_of(with_nothing_more), "Provider returned error");
    }
}
