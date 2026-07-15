use serde::Deserialize;
use std::collections::HashMap;

/// Provider configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ProviderConfig {
    Simple(SimpleProvider),
    OpenRouter(OpenRouterConfig),
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimpleProvider {
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterConfig {
    pub base_url: Option<String>,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProxyConfig {
    /// Listen address (default 127.0.0.1)
    #[serde(default = "default_bind")]
    pub bind: String,
    /// Listen port (default 3000)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Ordered list of upstream providers for failover.
    #[serde(default)]
    pub providers: Vec<NamedProvider>,
    /// Model remapping: client model → upstream model.
    #[serde(default)]
    pub model_map: HashMap<String, String>,
    /// Reasoning model override (used when request has thinking enabled).
    pub reasoning_model: Option<String>,
    /// Completion model override (used for standard requests).
    pub completion_model: Option<String>,
    /// Terms to strip from system prompts before forwarding.
    #[serde(default)]
    pub system_prompt_ignore_terms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamedProvider {
    pub name: String,
    #[serde(flatten)]
    pub config: ProviderConfig,
}

fn default_bind() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    3000
}

impl ProxyConfig {
    pub fn from_toml(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    pub fn default_with_openrouter(api_key: &str, model: &str) -> Self {
        Self {
            providers: vec![NamedProvider {
                name: "openrouter".into(),
                config: ProviderConfig::OpenRouter(OpenRouterConfig {
                    base_url: None,
                    api_key: api_key.into(),
                    model: model.into(),
                }),
            }],
            ..Self::default()
        }
    }
}
