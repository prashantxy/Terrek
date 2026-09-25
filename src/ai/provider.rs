use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    #[serde(alias = "claude")]
    #[value(alias = "claude")]
    Anthropic,
    /// OpenAI, or any OpenAI-compatible server via `base_url`.
    #[serde(rename = "openai")]
    #[value(name = "openai")]
    OpenAi,
    Gemini,
    Ollama,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 4] = [
        ProviderKind::Anthropic,
        ProviderKind::OpenAi,
        ProviderKind::Gemini,
        ProviderKind::Ollama,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "anthropic" | "claude" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAi),
            "gemini" | "google" => Some(Self::Gemini),
            "ollama" => Some(Self::Ollama),
            _ => None,
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-opus-5",
            Self::OpenAi => "gpt-4.1-mini",
            Self::Gemini => "gemini-2.5-flash",
            Self::Ollama => "llama3.2",
        }
    }

    /// Env vars checked (in order) for this provider's key.
    pub fn key_env_vars(self) -> &'static [&'static str] {
        match self {
            Self::Anthropic => &["ANTHROPIC_API_KEY"],
            Self::OpenAi => &["OPENAI_API_KEY"],
            Self::Gemini => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
            Self::Ollama => &[],
        }
    }

    pub fn needs_key(self) -> bool {
        !matches!(self, Self::Ollama)
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic (Claude)",
            Self::OpenAi => "OpenAI / OpenAI-compatible",
            Self::Gemini => "Google Gemini",
            Self::Ollama => "Ollama (local, no key)",
        }
    }

    /// Guess the provider from a pasted key's prefix.
    pub fn detect_from_key(key: &str) -> Option<Self> {
        let key = key.trim();
        if key.starts_with("sk-ant-") {
            Some(Self::Anthropic)
        } else if key.starts_with("sk-") {
            Some(Self::OpenAi)
        } else if key.starts_with("AIza") {
            Some(Self::Gemini)
        } else {
            None
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
            Self::Ollama => "ollama",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_key_prefixes() {
        assert_eq!(
            ProviderKind::detect_from_key("sk-ant-api03-x"),
            Some(ProviderKind::Anthropic)
        );
        assert_eq!(
            ProviderKind::detect_from_key("sk-proj-x"),
            Some(ProviderKind::OpenAi)
        );
        assert_eq!(
            ProviderKind::detect_from_key(" AIzaSy "),
            Some(ProviderKind::Gemini)
        );
        assert_eq!(ProviderKind::detect_from_key("xyz"), None);
    }

    #[test]
    fn parses_names_and_aliases() {
        assert_eq!(ProviderKind::parse("Claude"), Some(ProviderKind::Anthropic));
        assert_eq!(ProviderKind::parse("openai"), Some(ProviderKind::OpenAi));
        assert_eq!(ProviderKind::parse("nope"), None);
    }

    #[test]
    fn display_round_trips_through_parse() {
        for kind in ProviderKind::ALL {
            assert_eq!(ProviderKind::parse(&kind.to_string()), Some(kind));
        }
    }
}
