use keyring::Entry;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
}

impl AppConfig {
    fn config_path() -> Option<PathBuf> {
        let proj_dirs = match directories::ProjectDirs::from("com", "startux", "tuxtests") {
            Some(d) => d,
            None => {
                eprintln!("⚠️ CRITICAL Warning: `directories` crate failed to map $HOME or XDG native boundaries. Falling back to local state.");
                return Some(PathBuf::from("tuxtests_config.toml"));
            }
        };
        let dir = proj_dirs.config_dir();
        if !dir.exists() {
            if let Err(e) = fs::create_dir_all(dir) {
                eprintln!("⚠️ Warning: System /home partition restricted ({}). Falling back to local tuxtests_config.toml mapping.", e);
                return Some(PathBuf::from("tuxtests_config.toml"));
            }
        }
        Some(dir.join("config.toml"))
    }

    pub fn load() -> Self {
        let mut default = Self {
            provider: default_provider(),
            ollama_model: default_ollama_model(), // Strong offline default for deep reasoning capabilities
            ollama_url: default_ollama_url(),
        };

        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str::<Self>(&content) {
                    return config.normalized();
                }
            } else {
                default.save(); // Automatically bootstrap the filesystem boundary natively!
            }
        }
        default.normalized()
    }

    pub fn save(&mut self) -> bool {
        *self = self.clone().normalized();
        if let Some(path) = Self::config_path() {
            match toml::to_string(self) {
                Ok(content) => {
                    if let Err(e) = fs::write(&path, content) {
                        eprintln!("⚠️ CRITICAL Warning: Failed to physically flush TOML back to disk at {:?}. Error: {}", path, e);
                        return false;
                    }
                    return true;
                }
                Err(e) => {
                    eprintln!("⚠️ TOML Serialization failed natively! Error: {}", e);
                    return false;
                }
            }
        }
        false
    }

    /// Explicitly reaches into KWallet / Gnome Keyring / Secret Service to isolate keys safely from `env`.
    pub fn get_gemini_key() -> Option<String> {
        let entry = Entry::new("tuxtests", "gemini_api").ok()?;
        entry.get_password().ok()
    }

    pub fn normalized(mut self) -> Self {
        self.provider = normalize_provider(&self.provider).unwrap_or_else(|_| default_provider());
        self.ollama_model =
            normalize_ollama_model(&self.ollama_model).unwrap_or_else(|_| default_ollama_model());
        self.ollama_url =
            normalize_ollama_url(&self.ollama_url).unwrap_or_else(|_| default_ollama_url());
        self
    }
}

pub fn normalize_provider(input: &str) -> Result<String, String> {
    let provider = input.trim().to_lowercase();
    match provider.as_str() {
        "gemini" | "ollama" => Ok(provider),
        _ => Err(format!(
            "unsupported provider '{}'; expected 'gemini' or 'ollama'",
            input.trim()
        )),
    }
}

pub fn normalize_ollama_model(input: &str) -> Result<String, String> {
    let model = input.trim();
    if model.is_empty() {
        Err("ollama model cannot be empty".to_string())
    } else {
        Ok(model.to_string())
    }
}

pub fn normalize_ollama_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let mut parsed =
        Url::parse(trimmed).map_err(|e| format!("invalid Ollama URL '{}': {}", trimmed, e))?;

    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "invalid Ollama URL '{}': unsupported scheme '{}'",
                trimmed, scheme
            ));
        }
    }

    if parsed.host_str().is_none() {
        return Err(format!("invalid Ollama URL '{}': missing host", trimmed));
    }

    parsed.set_query(None);
    parsed.set_fragment(None);
    if parsed.path() != "/" {
        parsed.set_path("");
    }

    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

pub fn config_to_toml(config: &AppConfig) -> Result<String, toml::ser::Error> {
    toml::to_string(config)
}

pub fn config_from_toml(content: &str) -> Result<AppConfig, toml::de::Error> {
    toml::from_str::<AppConfig>(content).map(AppConfig::normalized)
}

fn default_provider() -> String {
    "gemini".to_string()
}

fn default_ollama_model() -> String {
    "mistral".to_string()
}

fn default_ollama_url() -> String {
    "http://127.0.0.1:11434".to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        config_from_toml, config_to_toml, default_ollama_model, default_ollama_url,
        normalize_ollama_model, normalize_ollama_url, normalize_provider, AppConfig,
    };

    #[test]
    fn normalizes_provider_values() {
        assert_eq!(normalize_provider(" Gemini ").unwrap(), "gemini");
        assert_eq!(normalize_provider("OLLAMA").unwrap(), "ollama");
        assert!(normalize_provider("openai").is_err());
    }

    #[test]
    fn normalizes_ollama_model_values() {
        assert_eq!(
            normalize_ollama_model(" gemma3:latest ").unwrap(),
            "gemma3:latest"
        );
        assert!(normalize_ollama_model("   ").is_err());
    }

    #[test]
    fn normalizes_ollama_url_values() {
        assert_eq!(
            normalize_ollama_url("http://localhost:11434/").unwrap(),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_ollama_url("https://example.com:443/api?x=1#frag").unwrap(),
            "https://example.com"
        );
        assert!(normalize_ollama_url("localhost:11434").is_err());
    }

    #[test]
    fn normalizes_legacy_config_values() {
        let config = AppConfig {
            provider: "BAD".to_string(),
            ollama_model: " ".to_string(),
            ollama_url: "".to_string(),
        }
        .normalized();

        assert_eq!(config.provider, "gemini");
        assert_eq!(config.ollama_model, default_ollama_model());
        assert_eq!(config.ollama_url, default_ollama_url());
    }

    #[test]
    fn round_trips_config_through_toml() {
        let config = AppConfig {
            provider: "ollama".to_string(),
            ollama_model: "gemma3".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
        };

        let toml = config_to_toml(&config).unwrap();
        let restored = config_from_toml(&toml).unwrap();

        assert_eq!(restored.provider, "ollama");
        assert_eq!(restored.ollama_model, "gemma3");
        assert_eq!(restored.ollama_url, "http://localhost:11434");
    }

    #[test]
    fn restores_defaults_when_legacy_toml_is_missing_new_fields() {
        let restored = config_from_toml(
            r#"
provider = "gemini"
ollama_model = "mistral"
"#,
        )
        .unwrap();

        assert_eq!(restored.provider, "gemini");
        assert_eq!(restored.ollama_model, "mistral");
        assert_eq!(restored.ollama_url, default_ollama_url());
    }
}
