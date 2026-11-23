//! The models that run on the musician's own computer, through Ollama.
//!
//! Ollama answers on this computer only. It has no account and no key, and what the musician makes never
//! leaves the computer. It speaks the shared chat protocol, so `chat_model` talks to it like any service.
//! Two things are asked for in Ollama's own way, because the chat protocol does not say them: which of the
//! installed models can use tools, and how much room Ollama gives a model's conversation.

use serde_json::{Value, json};

use crate::agent::ModelError;
use crate::model_http::{self, HTTP_OK, rejection};
use crate::providers::OfferedModel;

const CHAT_FOLDER: &str = "/v1";
const NOT_RUNNING: &str = "Ollama is not running on this computer. Open Ollama, then open this list again.";
const USES_TOOLS: &str = "tools";
const SEES_PICTURES: &str = "vision";
const NEWEST: &str = "latest";
/// What Ollama's own guide asks for at the least when a model works as an agent or with coding tools.
const ROOM_THE_WORK_WANTS: u64 = 64_000;
const TOKENS_IN_A_THOUSAND: u64 = 1000;

/// Every installed model that can do MP Forge's work. `chat_root` is where Ollama speaks the chat protocol.
pub fn models_installed(chat_root: &str) -> Result<Vec<OfferedModel>, ModelError> {
    let ollama = own_root(chat_root);
    let installed = asked(model_http::quick_client().get(format!("{ollama}/api/tags")).call())?;
    let details_of = |name: &str| {
        let about = json!({ "model": name }).to_string();
        asked(model_http::quick_client().post(format!("{ollama}/api/show")).header("content-type", "application/json").send(about))
    };
    let offered = names_in(&installed).into_iter().filter_map(|name| offered(&name, &details_of(&name).ok()?));
    Ok(offered.collect())
}

/// How many tokens Ollama gives the model's conversation, or nothing when it does not say. Ollama gives a
/// model what its settings and the computer's memory allow, often far less than the model could hold, and
/// says how much only of a model it has loaded. So the model is loaded first, as the first message to it
/// would have done anyway.
pub fn room_given_to(chat_root: &str, model: &str) -> Option<u64> {
    let ollama = own_root(chat_root);
    let load_it = json!({ "model": model, "stream": false }).to_string();
    asked(model_http::client().post(format!("{ollama}/api/generate")).header("content-type", "application/json").send(load_it)).ok()?;
    let loaded = asked(model_http::quick_client().get(format!("{ollama}/api/ps")).call()).ok()?;
    room_of(model, &loaded)
}

/// When the conversation of a model on this computer is tidied: with a quarter of its room still free, and
/// no later than the musician asked.
pub fn tidied_at(room: Option<u64>, asked: Option<u64>) -> Option<u64> {
    let with_a_quarter_free = room.map(|tokens| tokens / 4 * 3);
    match (with_a_quarter_free, asked) {
        (Some(for_its_room), Some(as_asked)) => Some(for_its_room.min(as_asked)),
        (for_its_room, as_asked) => for_its_room.or(as_asked),
    }
}

/// What the musician is told when the room is less than the work wants, and nothing when there is enough.
pub fn said_of_little_room(room: u64) -> Option<String> {
    let in_thousands = |tokens: u64| format!("{}K", (tokens + TOKENS_IN_A_THOUSAND / 2) / TOKENS_IN_A_THOUSAND);
    let is_little = room < ROOM_THE_WORK_WANTS;
    is_little.then(|| {
        format!(
            "Ollama gives this model room for {} tokens, and making a plugin goes better with {} or more: raise the context length in Ollama's settings if this computer has the memory for it",
            in_thousands(room),
            in_thousands(ROOM_THE_WORK_WANTS)
        )
    })
}

fn own_root(chat_root: &str) -> &str {
    chat_root.strip_suffix(CHAT_FOLDER).unwrap_or(chat_root)
}

fn room_of(model: &str, loaded: &Value) -> Option<u64> {
    let models = loaded["models"].as_array()?;
    let is_the_one = |running: &&Value| ["name", "model"].iter().filter_map(|field| running[*field].as_str()).any(|name| in_full(name) == in_full(model));
    models.iter().find(is_the_one)?["context_length"].as_u64()
}

/// A model named without a tag is its newest: "gemma4" is "gemma4:latest".
fn in_full(name: &str) -> String {
    let has_a_tag = name.rsplit('/').next().is_some_and(|last_part| last_part.contains(':'));
    if has_a_tag { name.to_string() } else { format!("{name}:{NEWEST}") }
}

fn asked(answer: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, ModelError> {
    let response = answer.map_err(|_| ModelError::Unreachable(NOT_RUNNING.to_string()))?;
    let status = response.status().as_u16();
    let reply = response.into_body().read_to_string().map_err(|error| ModelError::Unreachable(error.to_string()))?;
    if status != HTTP_OK {
        return Err(rejection(status, &reply));
    }
    serde_json::from_str(&reply).map_err(|error| ModelError::Garbled(error.to_string()))
}

fn names_in(installed: &Value) -> Vec<String> {
    let models = installed["models"].as_array().map(Vec::as_slice).unwrap_or_default();
    models.iter().filter_map(|model| model["name"].as_str()).map(str::to_string).collect()
}

/// A model is offered when Ollama says it uses tools. An older Ollama that does not say is taken at its word
/// later, when the model is asked to use them. How much its conversation holds is set in Ollama, not by the
/// model, so it is left unsaid here and asked for when work with the model starts.
fn offered(name: &str, details: &Value) -> Option<OfferedModel> {
    let can = |ability: &str| details["capabilities"].as_array().map(|abilities| abilities.iter().any(|known| known == ability));
    let model = OfferedModel { id: name.to_string(), name: name.to_string(), sees_pictures: can(SEES_PICTURES).unwrap_or(false), context_window: None, efforts: Vec::new() };
    can(USES_TOOLS).unwrap_or(true).then_some(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_http::test_support::{service_answering, yes_saying};

    #[test]
    fn the_installed_models_are_read_by_name() {
        let installed = json!({ "models": [{ "name": "qwen3:8b", "size": 5 }, { "name": "gemma4:latest" }] });

        assert_eq!(names_in(&installed), ["qwen3:8b", "gemma4:latest"]);
        assert_eq!(names_in(&json!({ "models": [] })), Vec::<String>::new());
    }

    #[test]
    fn only_a_model_that_uses_tools_is_offered() {
        let builder = json!({ "capabilities": ["completion", "tools", "vision"] });
        let talker = json!({ "capabilities": ["completion", "thinking"] });

        let offered_builder = offered("qwen3:8b", &builder).unwrap();

        assert_eq!((offered_builder.id.as_str(), offered_builder.sees_pictures, offered_builder.context_window), ("qwen3:8b", true, None));
        assert_eq!(offered("gemma4:latest", &talker), None);
    }

    #[test]
    fn a_model_whose_abilities_are_not_said_is_offered_and_asked_later() {
        assert!(offered("old-model", &json!({ "details": { "family": "llama" } })).is_some_and(|model| !model.sees_pictures));
    }

    /// Stands in for Ollama, which says yes to one asker after another with the next of these replies.
    fn ollama_answering(replies: &[&str]) -> String {
        let ollama = service_answering(replies.iter().map(|reply| yes_saying(reply)).collect());
        format!("{ollama}{CHAT_FOLDER}")
    }

    #[test]
    fn the_room_a_model_is_given_is_asked_of_ollama_once_the_model_is_loaded() {
        const LOADED: &str = r#"{"model":"gemma4:31b","done":true,"done_reason":"load"}"#;
        const RUNNING: &str = r#"{"models":[{"name":"qwen3:8b","model":"qwen3:8b","context_length":4096},{"name":"gemma4:31b","model":"gemma4:31b","context_length":32768}]}"#;

        assert_eq!(room_given_to(&ollama_answering(&[LOADED, RUNNING]), "gemma4:31b"), Some(32_768));
    }

    #[test]
    fn the_room_is_left_unsaid_when_ollama_does_not_say_it() {
        let nobody_listens_here = "http://127.0.0.1:9/v1";
        let another_is_loaded = json!({ "models": [{ "name": "qwen3:8b", "context_length": 4096 }] });
        let an_older_ollama = json!({ "models": [{ "name": "gemma4:31b" }] });

        assert_eq!(room_given_to(nobody_listens_here, "gemma4:31b"), None);
        assert_eq!(room_of("gemma4:31b", &another_is_loaded), None);
        assert_eq!(room_of("gemma4:31b", &an_older_ollama), None);
    }

    #[test]
    fn a_model_named_without_a_tag_is_its_newest() {
        let loaded = json!({ "models": [{ "name": "gemma4:latest", "model": "gemma4:latest", "context_length": 131_072 }] });
        let from_another_shelf = json!({ "models": [{ "name": "hf.co/lab/gemma4", "context_length": 8192 }] });

        assert_eq!(room_of("gemma4", &loaded), Some(131_072));
        assert_eq!(room_of("gemma4:31b", &loaded), None);
        assert_eq!(room_of("hf.co/lab/gemma4:latest", &from_another_shelf), Some(8192));
    }

    #[test]
    fn the_conversation_is_tidied_with_a_quarter_of_the_room_free_and_no_later_than_asked() {
        assert_eq!(tidied_at(Some(32_768), None), Some(24_576));
        assert_eq!(tidied_at(Some(32_768), Some(100_000)), Some(24_576));
        assert_eq!(tidied_at(Some(262_144), Some(100_000)), Some(100_000));
        assert_eq!(tidied_at(None, Some(100_000)), Some(100_000));
        assert_eq!(tidied_at(None, None), None);
    }

    #[test]
    fn the_musician_is_told_only_when_the_room_is_less_than_the_work_wants() {
        let told = said_of_little_room(32_768).unwrap();

        assert!(told.starts_with("Ollama gives this model room for 33K tokens, and making a plugin goes better with 64K or more"), "{told}");
        assert!(said_of_little_room(4096).unwrap().contains("room for 4K tokens"));
        assert_eq!(said_of_little_room(64_000), None);
        assert_eq!(said_of_little_room(262_144), None);
    }

    #[test]
    fn an_ollama_that_is_not_running_is_said_in_plain_words() {
        let nobody_listens_here = "http://127.0.0.1:9/v1";

        let problem = models_installed(nobody_listens_here).unwrap_err();

        assert!(problem.to_string().contains("Ollama is not running"), "{problem}");
    }
}
