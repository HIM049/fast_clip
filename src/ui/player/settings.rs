use crate::ui::player::model::AudioRail;

pub struct PlayerSettings {
    pub audio_ix: usize,
    pub audio_rails: Vec<AudioRail>,
}

impl PlayerSettings {
    pub fn default() -> Self {
        Self {
            audio_ix: 0,
            audio_rails: vec![],
        }
    }
}
