//! The AI services the workbench can reach, and the models each one offers today.
//!
//! Anthropic is reached in its own way, through `claude` and `claude_code`. Every other service here speaks the
//! same chat protocol, so one connection, `chat_model`, serves them all. The list of models is always asked of
//! the service itself, so it is never out of date.
//!
//! One of them is not a service at all: Ollama runs models on the musician's own computer, with no account
//! and no key, and nothing the musician makes leaves the computer.

use serde_json::Value;

use crate::agent::ModelError;
use crate::ai_settings::Effort;
use crate::model_http::{self, HTTP_OK, rejection};
use crate::ollama;

pub struct Provider {
    /// The word the workbench knows the service by.
    pub word: &'static str,
    pub name: &'static str,
    api_root: &'static str,
    /// Where a key is looked for when the musician has not given the workbench one.
    pub key_variable: &'static str,
    /// The web page where the musician makes a key, for the workbench to show them.
    pub keys_are_made_at: &'static str,
    pub lists_models_without_a_key: bool,
    /// What to assume of a model whose listing does not say whether it can look at pictures.
    models_usually_see_pictures: bool,
    /// Whether a streamed reply only says how many tokens it used when the request asks it to. A service that
    /// always says is not asked, because some of them refuse a request that holds anything they do not know.
    pub counts_tokens_only_when_asked: bool,
    /// Whether the musician can get a key by signing in on the service's own web page, with nothing to paste.
    pub signs_in_with_browser: bool,
    /// Whether the models run on the musician's own computer, where there is no account and no key.
    pub runs_on_this_computer: bool,
}

pub const ANTHROPIC: Provider = Provider {
    word: "anthropic",
    name: "Anthropic",
    api_root: "https://api.anthropic.com/v1",
    key_variable: "ANTHROPIC_API_KEY",
    keys_are_made_at: "console.anthropic.com/settings/keys",
    lists_models_without_a_key: false,
    models_usually_see_pictures: true,
    counts_tokens_only_when_asked: false,
    signs_in_with_browser: false,
    runs_on_this_computer: false,
};

const CHAT_PROVIDERS: [Provider; 7] = [
    Provider { word: "openai", name: "OpenAI", api_root: "https://api.openai.com/v1", key_variable: "OPENAI_API_KEY", keys_are_made_at: "platform.openai.com/api-keys", lists_models_without_a_key: false, models_usually_see_pictures: true, counts_tokens_only_when_asked: true, signs_in_with_browser: false, runs_on_this_computer: false },
    Provider { word: "google", name: "Google", api_root: "https://generativelanguage.googleapis.com/v1beta/openai", key_variable: "GEMINI_API_KEY", keys_are_made_at: "aistudio.google.com/apikey", lists_models_without_a_key: false, models_usually_see_pictures: true, counts_tokens_only_when_asked: true, signs_in_with_browser: false, runs_on_this_computer: false },
    Provider { word: "xai", name: "xAI", api_root: "https://api.x.ai/v1", key_variable: "XAI_API_KEY", keys_are_made_at: "console.x.ai", lists_models_without_a_key: false, models_usually_see_pictures: true, counts_tokens_only_when_asked: true, signs_in_with_browser: false, runs_on_this_computer: false },
    Provider { word: "mistral", name: "Mistral", api_root: "https://api.mistral.ai/v1", key_variable: "MISTRAL_API_KEY", keys_are_made_at: "console.mistral.ai/api-keys", lists_models_without_a_key: false, models_usually_see_pictures: false, counts_tokens_only_when_asked: false, signs_in_with_browser: false, runs_on_this_computer: false },
    Provider { word: "deepseek", name: "DeepSeek", api_root: "https://api.deepseek.com", key_variable: "DEEPSEEK_API_KEY", keys_are_made_at: "platform.deepseek.com/api_keys", lists_models_without_a_key: false, models_usually_see_pictures: false, counts_tokens_only_when_asked: true, signs_in_with_browser: false, runs_on_this_computer: false },
    Provider { word: "openrouter", name: "OpenRouter", api_root: "https://openrouter.ai/api/v1", key_variable: "OPENROUTER_API_KEY", keys_are_made_at: "openrouter.ai/settings/keys", lists_models_without_a_key: true, models_usually_see_pictures: false, counts_tokens_only_when_asked: true, signs_in_with_browser: true, runs_on_this_computer: false },
    Provider { word: "ollama", name: "Ollama (on this computer)", api_root: "http://localhost:11434/v1", key_variable: "", keys_are_made_at: "", lists_models_without_a_key: true, models_usually_see_pictures: false, counts_tokens_only_when_asked: true, signs_in_with_browser: false, runs_on_this_computer: true },
];

/// Words in a model's name that mark it as made for something other than conversation.
const NOT_FOR_CONVERSATION: [&str; 17] = [
    "embed", "tts", "whisper", "dall-e", "image", "audio", "realtime", "transcribe", "moderation", "sora", "veo", "imagen", "davinci", "babbage", "aqa", "ocr", "rerank",
];
/// Google lists each model under this folder, and takes its name without it.
const FOLDER_GOOGLE_LISTS_UNDER: &str = "models/";

/// Every service, with Anthropic first.
pub fn every_provider() -> impl Iterator<Item = &'static Provider> {
    std::iter::once(&ANTHROPIC).chain(CHAT_PROVIDERS.iter())
}

pub fn provider_called(word: &str) -> Option<&'static Provider> {
    every_provider().find(|provider| provider.word == word)
}

/// One model a service offers that can do the workbench's work: it holds a conversation and it uses tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferedModel {
    /// The name the service knows it by.
    pub id: String,
    /// The name to show the musician.
    pub name: String,
    pub sees_pictures: bool,
    /// How many tokens its conversation can hold, when the service's list says.
    pub context_window: Option<u64>,
    /// The levels of effort it can be told to put in, in MP Forge's own words for them. Empty when its
    /// service does not say, in which case it is never told.
    pub efforts: Vec<String>,
}

impl Provider {
    pub fn is_anthropic(&self) -> bool {
        self.word == ANTHROPIC.word
    }

    pub fn chat_address(&self) -> String {
        format!("{}/chat/completions", self.api_root)
    }

    /// Whether a model here can be told how hard to think. OpenRouter takes one setting for every model it
    /// reaches and says in its list which levels each accepts. The others each have a way of their own, or none.
    pub fn takes_effort(&self) -> bool {
        self.is_openrouter()
    }

    /// Whether a model here can be given the web to look things up on. OpenRouter runs the searches itself, for
    /// every model it reaches, and charges for each one.
    pub fn sells_web_lookups(&self) -> bool {
        self.is_openrouter()
    }

    fn is_openrouter(&self) -> bool {
        self.word == "openrouter"
    }

    pub fn needs_a_key(&self) -> bool {
        !self.runs_on_this_computer
    }

    pub fn key_in_environment(&self) -> Option<String> {
        std::env::var(self.key_variable).ok().filter(|key| !key.trim().is_empty())
    }

    /// How many tokens a model's conversation is given, where that is decided on this computer and not by the
    /// model. Only Ollama is asked. A service on the internet gives a model all it holds, and says how much
    /// when it lists the model.
    pub fn room_given_to(&self, model: &str) -> Option<u64> {
        self.runs_on_this_computer.then(|| ollama::room_given_to(self.api_root, model)).flatten()
    }

    /// Asks the service which models it offers today. Nothing about the musician or their plugins is sent.
    pub fn models_offered(&self, key: Option<&str>) -> Result<Vec<OfferedModel>, ModelError> {
        if self.runs_on_this_computer {
            return ollama::models_installed(self.api_root);
        }
        let mut request = model_http::quick_client().get(format!("{}/models", self.api_root));
        if let Some(key) = key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        let response = request.call().map_err(|error| ModelError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let reply = response.into_body().read_to_string().map_err(|error| ModelError::Unreachable(error.to_string()))?;
        if status != HTTP_OK {
            return Err(rejection(status, &reply));
        }
        let listing = serde_json::from_str::<Value>(&reply).map_err(|error| ModelError::Garbled(error.to_string()))?;
        Ok(self.models_in(&listing))
    }

    fn models_in(&self, listing: &Value) -> Vec<OfferedModel> {
        let listed = listing["data"].as_array().map(Vec::as_slice).unwrap_or_default();
        listed.iter().filter(|entry| uses_tools(entry)).filter_map(|entry| self.offered(entry)).collect()
    }

    fn offered(&self, entry: &Value) -> Option<OfferedModel> {
        let listed_as = entry["id"].as_str()?;
        let id = listed_as.strip_prefix(FOLDER_GOOGLE_LISTS_UNDER).unwrap_or(listed_as);
        let name = entry["name"].as_str().unwrap_or(id);
        let efforts = if self.takes_effort() { efforts_taken(&entry["reasoning"]) } else { Vec::new() };
        let offered = OfferedModel { id: id.to_string(), name: name.to_string(), sees_pictures: self.sees_pictures(entry), context_window: context_window_of(entry), efforts };
        is_for_conversation(id).then_some(offered)
    }

    fn sees_pictures(&self, entry: &Value) -> bool {
        match entry["architecture"]["input_modalities"].as_array() {
            Some(taken_in) => taken_in.iter().any(|kind| kind == "image"),
            None => entry["capabilities"]["vision"].as_bool().unwrap_or(self.models_usually_see_pictures),
        }
    }
}

/// The effort levels a listing says a model accepts, kept to the ones MP Forge offers. A model that reasons
/// and names no levels accepts them all. One the listing says nothing about reasoning for accepts none.
fn efforts_taken(reasoning: &Value) -> Vec<String> {
    let offered_here = [Effort::Low, Effort::Medium, Effort::High, Effort::ExtraHigh, Effort::Max].map(Effort::word);
    let accepts = |level: &str| reasoning["supported_efforts"].as_array().is_none_or(|levels| levels.iter().any(|known| known == level));
    if !reasoning.is_object() {
        return Vec::new();
    }
    offered_here.into_iter().filter(|level| accepts(level)).map(str::to_string).collect()
}

/// Services say how much a model's conversation can hold under different names, and some do not say at all.
/// A model offered by several hosts is taken at the smallest any of them gives it.
fn context_window_of(entry: &Value) -> Option<u64> {
    let said_outright = ["context_length", "max_context_length", "context_window"].iter().find_map(|name| entry[*name].as_u64());
    let said_by_hosts = || entry["providers"].as_array()?.iter().filter_map(|host| host["context_length"].as_u64()).min();
    said_outright.or_else(said_by_hosts)
}

/// Services say whether a model uses tools in different places. One that does not say is taken at its word later,
/// when it is asked to use them.
fn uses_tools(entry: &Value) -> bool {
    if let Some(parameters) = entry["supported_parameters"].as_array() {
        return parameters.iter().any(|parameter| parameter == "tools");
    }
    if let Some(hosts) = entry["providers"].as_array() {
        return hosts.iter().any(|host| host["supports_tools"] == true);
    }
    entry["capabilities"]["function_calling"].as_bool().unwrap_or(true)
}

fn is_for_conversation(id: &str) -> bool {
    let lowered = id.to_lowercase();
    !NOT_FOR_CONVERSATION.iter().any(|marker| lowered.contains(marker))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn names_offered(provider_word: &str, listing: Value) -> Vec<String> {
        provider_called(provider_word).unwrap().models_in(&listing).into_iter().map(|model| model.id).collect()
    }

    #[test]
    fn every_service_is_found_by_its_word_and_anthropic_comes_first() {
        let words: Vec<&str> = every_provider().map(|provider| provider.word).collect();

        assert_eq!(words, ["anthropic", "openai", "google", "xai", "mistral", "deepseek", "openrouter", "ollama"]);
        assert!(provider_called("openrouter").is_some_and(|provider| provider.lists_models_without_a_key && provider.signs_in_with_browser));
        assert!(provider_called("somebody-else").is_none());
    }

    #[test]
    fn only_openrouter_sells_web_lookups() {
        let sellers: Vec<&str> = every_provider().filter(|provider| provider.sells_web_lookups()).map(|provider| provider.word).collect();

        assert_eq!(sellers, ["openrouter"]);
    }

    #[test]
    fn only_models_that_use_tools_are_offered_from_a_listing_that_says_which_do() {
        let listing = json!({ "data": [
            { "id": "acme/builder", "name": "Acme: Builder", "context_length": 400_000, "supported_parameters": ["tools", "temperature"], "architecture": { "input_modalities": ["text", "image"] } },
            { "id": "acme/talker", "name": "Acme: Talker", "supported_parameters": ["temperature"] },
        ] });

        let offered = provider_called("openrouter").unwrap().models_in(&listing);

        assert_eq!(offered, [OfferedModel { id: "acme/builder".to_string(), name: "Acme: Builder".to_string(), sees_pictures: true, context_window: Some(400_000), efforts: Vec::new() }]);
    }

    #[test]
    fn a_model_is_offered_with_the_effort_levels_its_listing_says_it_accepts() {
        let listing = json!({ "data": [
            { "id": "lab/thinker", "supported_parameters": ["tools"], "reasoning": { "mandatory": true, "supported_efforts": ["xhigh", "high", "medium", "low", "minimal"] } },
            { "id": "lab/any-effort", "supported_parameters": ["tools"], "reasoning": { "supported_efforts": null } },
            { "id": "lab/plain", "supported_parameters": ["tools"] },
            { "id": "lab/unsaid", "supported_parameters": ["tools"], "reasoning": null },
        ] });

        let efforts = |provider: &str| provider_called(provider).unwrap().models_in(&listing).into_iter().map(|model| model.efforts.join(" ")).collect::<Vec<_>>();

        assert_eq!(efforts("openrouter"), ["low medium high xhigh", "low medium high xhigh max", "", ""]);
        assert_eq!(efforts("openai"), ["", "", "", ""], "only a service that takes the setting has its models told");
    }

    #[test]
    fn a_model_hosted_in_several_places_is_offered_when_any_of_them_gives_it_tools() {
        let listing = json!({ "data": [
            { "id": "Lab/Open-Model", "providers": [{ "supports_tools": false, "context_length": 131_072 }, { "supports_tools": true, "context_length": 32_768 }], "architecture": { "input_modalities": ["text"] } },
            { "id": "Lab/No-Tools", "providers": [{ "status": "live" }] },
        ] });

        let offered = provider_called("openrouter").unwrap().models_in(&listing);

        assert_eq!(offered, [OfferedModel { id: "Lab/Open-Model".to_string(), name: "Lab/Open-Model".to_string(), sees_pictures: false, context_window: Some(32_768), efforts: Vec::new() }]);
    }

    #[test]
    fn models_made_for_something_other_than_conversation_are_left_out() {
        let listing = json!({ "data": [{ "id": "talk-large" }, { "id": "text-embedding-3" }, { "id": "talk-realtime" }, { "id": "TTS-1" }] });

        assert_eq!(names_offered("openai", listing), ["talk-large"]);
    }

    #[test]
    fn googles_models_are_known_by_their_names_without_the_folder() {
        let listing = json!({ "data": [{ "id": "models/talker-pro" }, { "id": "models/embedding-001" }] });

        assert_eq!(names_offered("google", listing), ["talker-pro"]);
    }

    #[test]
    fn a_service_that_says_which_models_see_pictures_is_believed_over_the_usual_guess() {
        let listing = json!({ "data": [{ "id": "seer", "capabilities": { "vision": true, "function_calling": true } }, { "id": "reader", "capabilities": { "function_calling": true } }, { "id": "mute", "capabilities": { "function_calling": false } }] });

        let offered = provider_called("mistral").unwrap().models_in(&listing);

        assert_eq!(offered.iter().map(|model| (model.id.as_str(), model.sees_pictures)).collect::<Vec<_>>(), [("seer", true), ("reader", false)]);
    }

    #[test]
    #[ignore = "asks the real services"]
    fn the_public_listings_can_really_be_read() {
        let offered = provider_called("openrouter").unwrap().models_offered(None).unwrap();
        println!("openrouter: {} models, first {:?}", offered.len(), offered.first());
        assert!(offered.len() > 20);
    }

    #[test]
    fn only_the_models_on_this_computer_need_no_key() {
        let keyless: Vec<&str> = every_provider().filter(|provider| !provider.needs_a_key()).map(|provider| provider.word).collect();

        assert_eq!(keyless, ["ollama"]);
    }

    #[test]
    fn the_chat_address_is_built_from_the_services_own_root() {
        assert_eq!(provider_called("deepseek").unwrap().chat_address(), "https://api.deepseek.com/chat/completions");
        assert_eq!(provider_called("ollama").unwrap().chat_address(), "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn only_a_service_on_this_computer_is_asked_how_much_room_it_gives_a_model() {
        let on_the_internet = CHAT_PROVIDERS.iter().filter(|provider| !provider.runs_on_this_computer);

        assert!(on_the_internet.clone().count() > 1);
        assert!(on_the_internet.clone().all(|provider| provider.room_given_to("any-model").is_none()));
    }
}
