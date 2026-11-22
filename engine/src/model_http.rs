//! What every AI connection over the internet has in common: one way to ask, to wait, to try again,
//! and to say why a service said no.

use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::agent::ModelError;

const REQUEST_TIME_LIMIT: Duration = Duration::from_secs(20 * 60);
const WAITS_BEFORE_RETRYING: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(8)];
/// A question with a short answer, such as which models a service offers, is not waited on for long.
const QUICK_TIME_LIMIT: Duration = Duration::from_secs(30);
/// Long enough to fetch a few hundred megabytes over an ordinary line, and no longer.
const DOWNLOAD_TIME_LIMIT: Duration = Duration::from_secs(10 * 60);
pub const HTTP_OK: u16 = 200;
const HTTP_TOO_MANY_REQUESTS: u16 = 429;

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

pub fn is_worth_retrying(error: &ModelError) -> bool {
    match error {
        ModelError::Rejected { status, .. } => *status == HTTP_TOO_MANY_REQUESTS || *status >= 500,
        ModelError::Unreachable(_) => true,
        ModelError::NoCredential(_) | ModelError::Garbled(_) => false,
    }
}

/// A refusal, with the reason the service gave when it gave one in a form that can be read.
pub fn rejection(status: u16, reply: &str) -> ModelError {
    let stated_reason = serde_json::from_str::<Value>(reply).ok().and_then(|parsed| reason_in(&parsed));
    ModelError::Rejected { status, message: stated_reason.unwrap_or_else(|| reply.trim().to_string()) }
}

/// Services word a refusal differently. These are the places the reason has been seen.
fn reason_in(reply: &Value) -> Option<String> {
    let places = [&reply["error"]["message"], &reply["error"], &reply["detail"], &reply["message"]];
    places.into_iter().find_map(|place| place.as_str().map(str::to_string))
}

#[cfg(test)]
mod tests {
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
}
