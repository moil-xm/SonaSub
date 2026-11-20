use std::fs;

use serde::{Deserialize, Serialize};

use crate::config::normalize_openai_base_url;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderIcon {
    pub name: String,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub icon: ProviderIcon,
    pub models: Vec<String>,
    pub concurrency: usize,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_url: String::new(),
            api_key: String::new(),
            icon: ProviderIcon::default(),
            models: Vec::new(),
            concurrency: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub providers: Vec<ProviderConfig>,
    pub load_error: Option<String>,
}

fn normalize_provider(mut provider: ProviderConfig) -> Result<ProviderConfig, String> {
    provider.name = provider.name.trim().to_owned();
    provider.api_key = provider.api_key.trim().to_owned();
    provider.base_url = normalize_openai_base_url(&provider.base_url)?;
    provider.icon.name = provider.icon.name.trim().to_owned();
    provider.icon.data_url = provider.icon.data_url.trim().to_owned();
    let mut models = Vec::new();
    for model in provider.models {
        let model = model.trim().to_owned();
        if !model.is_empty() && !models.contains(&model) {
            models.push(model);
        }
    }
    provider.models = models;
    if provider.concurrency == 0 || provider.concurrency > 16 {
        return Err("Concurrent requests must be between 1 and 16.".to_owned());
    }
    Ok(provider)
}

pub fn sanitize_provider(provider: ProviderConfig) -> Result<ProviderConfig, String> {
    let provider = normalize_provider(provider)?;
    if provider.name.is_empty() {
        return Err("Provider name must not be empty.".to_owned());
    }
    Ok(provider)
}

pub fn list_providers(path: &std::path::Path) -> ProviderView {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ProviderView {
                providers: Vec::new(),
                load_error: None,
            };
        }
        Err(error) => {
            return ProviderView {
                providers: Vec::new(),
                load_error: Some(format!("Failed to read providers: {error}")),
            };
        }
    };
    match serde_json::from_str::<Vec<ProviderConfig>>(&text) {
        Ok(providers) => ProviderView {
            providers: providers
                .into_iter()
                .filter_map(|provider| normalize_provider(provider).ok())
                .collect(),
            load_error: None,
        },
        Err(error) => ProviderView {
            providers: Vec::new(),
            load_error: Some(format!("Failed to parse providers: {error}")),
        },
    }
}

pub fn save_provider(
    path: &std::path::Path,
    provider: ProviderConfig,
) -> Result<ProviderView, String> {
    let provider = sanitize_provider(provider)?;
    let mut view = list_providers(path);
    view.providers
        .retain(|current| current.name != provider.name);
    view.providers.push(provider);
    let parent = path
        .parent()
        .ok_or_else(|| "The provider path has no parent directory.".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create the provider directory: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(&view.providers)
        .map_err(|error| format!("Failed to serialize providers: {error}"))?;
    fs::write(&temporary, json).map_err(|error| format!("Failed to write providers: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("Failed to replace providers: {error}"))?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("sonasub-providers-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn saves_a_provider_without_any_models() {
        let directory = temp_dir();
        let path = directory.join("providers.json");
        let provider = ProviderConfig {
            name: "OpenAI".to_owned(),
            base_url: " https://example.test/v1/ ".to_owned(),
            ..ProviderConfig::default()
        };

        save_provider(&path, provider).unwrap();

        let view = list_providers(&path);
        assert!(view.load_error.is_none());
        assert_eq!(view.providers.len(), 1);
        assert_eq!(view.providers[0].base_url, "https://example.test/v1");
        assert!(view.providers[0].models.is_empty());
        assert_eq!(view.providers[0].concurrency, 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_an_empty_provider_name() {
        let error = sanitize_provider(ProviderConfig {
            base_url: "https://example.test/v1".to_owned(),
            ..ProviderConfig::default()
        })
        .unwrap_err();
        assert!(error.contains("name"));
    }
}
