use std::{env, fs, path::PathBuf};

use gpui::Global;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

impl Language {
    pub fn as_locale(self) -> String {
        serde_to_string(self).unwrap_or_else(|| "en".to_string())
    }

    pub fn from_locale(locale: &str) -> Option<Self> {
        serde_from_string(locale)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "lowercase")]
pub struct AppConfig {
    pub language: Language,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::En,
        }
    }
}

impl Global for AppConfig {}

pub fn load() -> AppConfig {
    let Ok(path) = config_path() else {
        return AppConfig::default();
    };
    println!("Read config from {:?}", path);

    let Ok(content) = fs::read_to_string(path) else {
        return AppConfig::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save(config: &AppConfig) -> anyhow::Result<()> {
    let path = config_path()?;
    println!("Save config to {:?}", path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

fn config_path() -> anyhow::Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

#[cfg(target_os = "windows")]
fn config_dir() -> anyhow::Result<PathBuf> {
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("APPDATA is not set"))?;
    Ok(base.join("fast_clip"))
}

#[cfg(target_os = "macos")]
fn config_dir() -> anyhow::Result<PathBuf> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join("fast_clip"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn config_dir() -> anyhow::Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path).join("fast_clip"));
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(home.join(".config").join("fast_clip"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
fn config_dir() -> anyhow::Result<PathBuf> {
    Ok(env::current_dir()?.join("fast_clip"))
}

fn serde_to_string<T: Serialize>(value: T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
}

fn serde_from_string<T: DeserializeOwned>(value: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).ok()
}
