//! The models that run on the musician's own computer, through Ollama.
//!
//! Ollama answers on this computer only. It has no account and no key, and what the musician makes never
//! leaves the computer. It speaks the shared chat protocol, so `chat_model` talks to it like any service.
//! Only the list of models is asked for in Ollama's own way, because that is where it says which of them
//! can use tools.

use serde_json::{Value, json};

use crate::agent::ModelError;
use crate::model_http::{self, HTTP_OK, rejection};
use crate::providers::OfferedModel;

const CHAT_FOLDER: &str = "/v1";
const NOT_RUNNING: &str = "Ollama is not running on this computer. Open Ollama, then open this list again.";
const USES_TOOLS: &str = "tools";
const SEES_PICTURES: &str = "vision";

/// Every installed model that can do MP Forge's work. `chat_root` is where Ollama speaks the chat protocol.
pub fn models_installed(chat_root: &str) -> Result<Vec<OfferedModel>, ModelError> {
    let ollama = chat_root.strip_suffix(CHAT_FOLDER).unwrap_or(chat_root);
    let installed = asked(model_http::quick_client().get(format!("{ollama}/api/tags")).call())?;
    let details_of = |name: &str| {
        let about = json!({ "model": name }).to_string();
        asked(model_http::quick_client().post(format!("{ollama}/api/show")).header("content-type", "application/json").send(about))
    };
    let offered = names_in(&installed).into_iter().filter_map(|name| offered(&name, &details_of(&name).ok()?));
    Ok(offered.collect())
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
/// model, so it is left unsaid.
fn offered(name: &str, details: &Value) -> Option<OfferedModel> {
    let can = |ability: &str| details["capabilities"].as_array().map(|abilities| abilities.iter().any(|known| known == ability));
    let model = OfferedModel { id: name.to_string(), name: name.to_string(), sees_pictures: can(SEES_PICTURES).unwrap_or(false), context_window: None, efforts: Vec::new() };
    can(USES_TOOLS).unwrap_or(true).then_some(model)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn an_ollama_that_is_not_running_is_said_in_plain_words() {
        let nobody_listens_here = "http://127.0.0.1:9/v1";

        let problem = models_installed(nobody_listens_here).unwrap_err();

        assert!(problem.to_string().contains("Ollama is not running"), "{problem}");
    }
}
