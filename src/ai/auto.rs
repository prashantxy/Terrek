//! Pick the provider and key automatically: explicit config first, then
//! whichever well-known API key env var is set.

use anyhow::{bail, Result};

use super::provider::ProviderKind;
use super::{anthropic, gemini, ollama, openai, ChatModel};
use crate::config::AiConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub provider: ProviderKind,
    pub model: String,
    pub api_key: Option<String>,
    /// Where the key came from, for `doctor` / `config show`.
    pub key_source: Option<String>,
    pub base_url: Option<String>,
}

pub fn resolve(cfg: &AiConfig) -> Result<Resolved> {
    resolve_with(cfg, |name| std::env::var(name).ok())
}

pub fn resolve_with(cfg: &AiConfig, env: impl Fn(&str) -> Option<String>) -> Result<Resolved> {
    let env = |name: &str| env(name).filter(|v| !v.trim().is_empty());

    let provider = match cfg.provider {
        Some(p) => p,
        None => {
            let from_key = cfg
                .api_key
                .as_deref()
                .and_then(ProviderKind::detect_from_key);
            let from_env = || {
                ProviderKind::ALL
                    .into_iter()
                    .find(|p| p.key_env_vars().iter().any(|v| env(v).is_some()))
            };
            match from_key.or_else(from_env) {
                Some(p) => p,
                None => bail!(
                    "no AI provider configured. Run `terrek setup`, or export ANTHROPIC_API_KEY, \
                     OPENAI_API_KEY, or GEMINI_API_KEY"
                ),
            }
        }
    };

    let (api_key, key_source) = if !provider.needs_key() {
        (None, None)
    } else if let Some(key) = cfg.api_key.clone().filter(|k| !k.trim().is_empty()) {
        (Some(key), Some("config file".to_string()))
    } else if let Some(key) = env("TERREK_API_KEY") {
        (Some(key), Some("$TERREK_API_KEY".to_string()))
    } else if let Some((var, key)) = provider
        .key_env_vars()
        .iter()
        .find_map(|v| env(v).map(|k| (*v, k)))
    {
        (Some(key), Some(format!("${var}")))
    } else {
        bail!(
            "no API key for {provider}. Run `terrek setup` or export {}",
            provider.key_env_vars().join(" / ")
        );
    };

    Ok(Resolved {
        provider,
        model: cfg
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model().to_string()),
        api_key,
        key_source,
        base_url: cfg.base_url.clone(),
    })
}

pub fn build(resolved: Resolved) -> Result<Box<dyn ChatModel>> {
    let key = resolved.api_key.unwrap_or_default();
    Ok(match resolved.provider {
        ProviderKind::Anthropic => Box::new(anthropic::Anthropic::new(
            key,
            resolved.model,
            resolved.base_url,
        )?),
        ProviderKind::OpenAi => {
            Box::new(openai::OpenAi::new(key, resolved.model, resolved.base_url)?)
        }
        ProviderKind::Gemini => {
            Box::new(gemini::Gemini::new(key, resolved.model, resolved.base_url)?)
        }
        ProviderKind::Ollama => Box::new(ollama::Ollama::new(resolved.model, resolved.base_url)?),
    })
}

pub fn from_config(cfg: &AiConfig) -> Result<Box<dyn ChatModel>> {
    build(resolve(cfg)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn infers_provider_from_env() {
        let r = resolve_with(&AiConfig::default(), env_of(&[("OPENAI_API_KEY", "sk-1")])).unwrap();
        assert_eq!(r.provider, ProviderKind::OpenAi);
        assert_eq!(r.api_key.as_deref(), Some("sk-1"));
        assert_eq!(r.model, "gpt-4.1-mini");
        assert_eq!(r.key_source.as_deref(), Some("$OPENAI_API_KEY"));
    }

    #[test]
    fn config_key_beats_env() {
        let cfg = AiConfig {
            api_key: Some("sk-ant-cfg".into()),
            ..Default::default()
        };
        let r = resolve_with(&cfg, env_of(&[("ANTHROPIC_API_KEY", "sk-ant-env")])).unwrap();
        assert_eq!(r.provider, ProviderKind::Anthropic);
        assert_eq!(r.api_key.as_deref(), Some("sk-ant-cfg"));
    }

    #[test]
    fn ollama_needs_no_key() {
        let cfg = AiConfig {
            provider: Some(ProviderKind::Ollama),
            ..Default::default()
        };
        let r = resolve_with(&cfg, env_of(&[])).unwrap();
        assert_eq!(r.api_key, None);
    }

    #[test]
    fn missing_everything_is_a_helpful_error() {
        let err = resolve_with(&AiConfig::default(), env_of(&[])).unwrap_err();
        assert!(err.to_string().contains("terrek setup"));
    }

    #[test]
    fn empty_env_values_are_ignored() {
        let cfg = AiConfig {
            provider: Some(ProviderKind::Gemini),
            ..Default::default()
        };
        let r = resolve_with(
            &cfg,
            env_of(&[("GEMINI_API_KEY", ""), ("GOOGLE_API_KEY", "AIza1")]),
        )
        .unwrap();
        assert_eq!(r.api_key.as_deref(), Some("AIza1"));
    }
}
