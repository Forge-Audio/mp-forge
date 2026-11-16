//! Which Claude model does the work, how hard it thinks, and whether it may look things up online.
//! All three are the user's choice.
//!
//! Leaving the model or the effort unset means "whatever this connection normally uses".

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeModel {
    Fable,
    Opus,
    Sonnet,
    Haiku,
}

impl ClaudeModel {
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "fable" => Some(ClaudeModel::Fable),
            "opus" => Some(ClaudeModel::Opus),
            "sonnet" => Some(ClaudeModel::Sonnet),
            "haiku" => Some(ClaudeModel::Haiku),
            _ => None,
        }
    }

    /// The name Anthropic's service and Claude Code both know the model by.
    pub fn id(self) -> &'static str {
        match self {
            ClaudeModel::Fable => "claude-fable-5-2",
            ClaudeModel::Opus => "claude-opus-5-5",
            ClaudeModel::Sonnet => "claude-sonnet-5-5",
            ClaudeModel::Haiku => "claude-haiku-5-5",
        }
    }

    /// Whether Anthropic can hand a declined request to another Claude model inside the same call.
    pub fn has_server_side_fallback(self) -> bool {
        self != ClaudeModel::Haiku
    }

    /// Whether the model takes Anthropic's newer web tools, which filter what they find before the model reads it.
    pub fn has_filtering_web_tools(self) -> bool {
        self != ClaudeModel::Haiku
    }
}

/// How much reasoning the model puts in. More is slower and uses more of the user's allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
    ExtraHigh,
    Max,
}

impl Effort {
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "low" => Some(Effort::Low),
            "medium" => Some(Effort::Medium),
            "high" => Some(Effort::High),
            "xhigh" => Some(Effort::ExtraHigh),
            "max" => Some(Effort::Max),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::ExtraHigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiSettings {
    pub model: Option<ClaudeModel>,
    pub effort: Option<Effort>,
    /// Whether the AI may search the web and read pages. Reading is all it can do there.
    pub web_access: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownSetting(String);

impl fmt::Display for UnknownSetting {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` is not a choice. Models: fable, opus, sonnet, haiku. Reasoning: low, medium, high, xhigh, max.",
            self.0
        )
    }
}

impl AiSettings {
    /// Reads the choices as typed or picked. Nothing, or the word "default", leaves a setting unset.
    pub fn from_words(model: Option<&str>, effort: Option<&str>) -> Result<Self, UnknownSetting> {
        Ok(Self { model: chosen(model, ClaudeModel::from_word)?, effort: chosen(effort, Effort::from_word)?, web_access: false })
    }

    pub fn with_web_access(mut self, allowed: bool) -> Self {
        self.web_access = allowed;
        self
    }
}

fn chosen<T>(word: Option<&str>, read: fn(&str) -> Option<T>) -> Result<Option<T>, UnknownSetting> {
    match word {
        None | Some("default") | Some("") => Ok(None),
        Some(word) => read(word).map(Some).ok_or_else(|| UnknownSetting(word.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_are_read_from_plain_words() {
        let settings = AiSettings::from_words(Some("sonnet"), Some("xhigh")).unwrap();

        assert_eq!(settings, AiSettings { model: Some(ClaudeModel::Sonnet), effort: Some(Effort::ExtraHigh), web_access: false });
        assert_eq!(settings.model.unwrap().id(), "claude-sonnet-5-5");
        assert_eq!(settings.effort.unwrap().word(), "xhigh");
    }

    #[test]
    fn nothing_or_default_leaves_a_setting_unset() {
        assert_eq!(AiSettings::from_words(None, Some("default")).unwrap(), AiSettings::default());
    }

    #[test]
    fn the_web_stays_closed_until_it_is_opened() {
        let settings = AiSettings::from_words(None, None).unwrap();

        assert!(!settings.web_access);
        assert!(settings.with_web_access(true).web_access);
    }

    #[test]
    fn an_unknown_word_is_refused_and_named() {
        let refused = AiSettings::from_words(Some("gpt"), None).unwrap_err();

        assert!(refused.to_string().contains("`gpt`"));
    }
}
