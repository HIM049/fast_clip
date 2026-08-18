use gpui::Global;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use strum_macros::EnumIter;

use super::utils::{load_json, save_json};

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum GpuPolicy {
    SoftwareOnly,
    PreferIntegrated,
    PreferDiscrete,
}
impl GpuPolicy {
    pub fn value(self) -> String {
        serde_to_string(self).expect("GpuPolicy must serialize to a string")
    }

    pub fn from_value(value: &str) -> Option<Self> {
        serde_from_string(value)
    }

    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::SoftwareOnly => "settings.gpu_policy.software_only",
            Self::PreferIntegrated => "settings.gpu_policy.prefer_integrated",
            Self::PreferDiscrete => "settings.gpu_policy.prefer_discrete",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum StepMode {
    Percent,
    Second,
}

impl StepMode {
    pub fn value(self) -> String {
        serde_to_string(self).expect("StepMode must serialize to a string")
    }

    pub fn from_value(value: &str) -> Option<Self> {
        serde_from_string(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct AppConfig {
    pub language: Language,
    pub check_update: bool,
    pub gpu_policy: GpuPolicy,
    pub step_mode: StepMode,
    pub step_percent: f64,
    pub step_sec: f64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::En,
            check_update: true,
            gpu_policy: GpuPolicy::PreferDiscrete,
            step_mode: StepMode::Percent,
            step_percent: 0.01,
            step_sec: 5.0,
        }
    }
}

impl AppConfig {
    pub fn handle_seek(&self, now: f64, duration: f64, forward: bool) -> f64 {
        let step = match self.step_mode {
            StepMode::Percent => duration * self.step_percent,
            StepMode::Second => self.step_sec,
        };

        if forward { now + step } else { now - step }
    }

    pub fn save(&self) -> Option<anyhow::Error> {
        if let Err(e) = save(self) {
            println!("failed to save config: {}", e);
            return Some(e);
        }
        None
    }
}

impl Global for AppConfig {}

pub fn load() -> AppConfig {
    load_json("config.json")
}

pub fn save(config: &AppConfig) -> anyhow::Result<()> {
    save_json("config.json", config)
}

fn serde_to_string<T: Serialize>(value: T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
}

fn serde_from_string<T: DeserializeOwned>(value: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(value.to_string())).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_seek_uses_percent_of_duration_in_percent_mode() {
        let config = AppConfig {
            step_mode: StepMode::Percent,
            step_percent: 0.05,
            ..Default::default()
        };

        assert_eq!(config.handle_seek(20.0, 200.0, true), 30.0);
        assert_eq!(config.handle_seek(20.0, 200.0, false), 10.0);
    }

    #[test]
    fn handle_seek_uses_fixed_seconds_in_second_mode() {
        let config = AppConfig {
            step_mode: StepMode::Second,
            step_sec: 7.5,
            ..Default::default()
        };

        assert_eq!(config.handle_seek(20.0, 200.0, true), 27.5);
        assert_eq!(config.handle_seek(20.0, 200.0, false), 12.5);
    }
}
