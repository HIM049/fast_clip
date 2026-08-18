pub mod keybinding;
pub mod settings;
mod utils;

pub fn load_all() -> (settings::AppConfig, keybinding::KeymapConfig) {
    (settings::load(), keybinding::load())
}
