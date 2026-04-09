use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub provider: String,
    pub ollama_model: String,
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
            provider: "gemini".to_string(),
            ollama_model: "mistral".to_string(), // Strong offline default for deep reasoning capabilities
        };

        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            } else {
                default.save(); // Automatically bootstrap the filesystem boundary natively!
            }
        }
        default
    }

    pub fn save(&mut self) -> bool {
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
}
