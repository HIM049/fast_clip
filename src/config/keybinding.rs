use gpui::{Global, Keystroke};
use serde::{Deserialize, Serialize};

use super::utils::{load_json, save_json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct KeymapConfig {
    pub play_pause: String,
    pub seek_backward: String,
    pub seek_forward: String,
    pub volume_up: String,
    pub volume_down: String,
    pub set_range_start: String,
    pub set_range_end: String,
    pub export: String,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            play_pause: "space".into(),
            seek_backward: "left".into(),
            seek_forward: "right".into(),
            volume_up: "up".into(),
            volume_down: "down".into(),
            set_range_start: "[".into(),
            set_range_end: "]".into(),
            export: default_export().into(),
        }
    }
}

#[cfg(target_os = "macos")]
fn default_export() -> &'static str {
    "cmd-s"
}

#[cfg(not(target_os = "macos"))]
fn default_export() -> &'static str {
    "ctrl-s"
}

impl KeymapConfig {
    pub fn save(&self) -> Option<anyhow::Error> {
        if let Err(e) = save(self) {
            println!("failed to save keymap config: {}", e);
            return Some(e);
        }
        None
    }
}

impl Global for KeymapConfig {}

pub fn load() -> KeymapConfig {
    let mut keymap: KeymapConfig = load_json("keybinding.json");
    let defaults = KeymapConfig::default();

    validate_binding("play_pause", &mut keymap.play_pause, &defaults.play_pause);
    validate_binding(
        "seek_backward",
        &mut keymap.seek_backward,
        &defaults.seek_backward,
    );
    validate_binding(
        "seek_forward",
        &mut keymap.seek_forward,
        &defaults.seek_forward,
    );
    validate_binding("volume_up", &mut keymap.volume_up, &defaults.volume_up);
    validate_binding(
        "volume_down",
        &mut keymap.volume_down,
        &defaults.volume_down,
    );
    validate_binding(
        "set_range_start",
        &mut keymap.set_range_start,
        &defaults.set_range_start,
    );
    validate_binding(
        "set_range_end",
        &mut keymap.set_range_end,
        &defaults.set_range_end,
    );
    validate_binding("export", &mut keymap.export, &defaults.export);

    keymap
}

pub fn save(keymap: &KeymapConfig) -> anyhow::Result<()> {
    save_json("keybinding.json", keymap)
}

fn validate_binding(name: &str, binding: &mut String, default: &str) {
    if Keystroke::parse(binding).is_err() {
        println!("invalid keybinding for {name}: {binding:?}, falling back to default {default:?}");
        binding.clear();
        binding.push_str(default);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_binding_falls_back_to_default() {
        let defaults = KeymapConfig::default();
        let mut binding = "foo-bar".to_string();

        validate_binding("play_pause", &mut binding, &defaults.play_pause);

        assert_eq!(binding, defaults.play_pause);
    }

    #[test]
    fn valid_binding_is_kept() {
        let defaults = KeymapConfig::default();
        let mut binding = "ctrl-p".to_string();

        validate_binding("play_pause", &mut binding, &defaults.play_pause);

        assert_eq!(binding, "ctrl-p");
    }
}
