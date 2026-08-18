//! keyboard shortcut definitions

pub const PLAY_PAUSE: &str = "space";
pub const SEEK_BACKWARD: &str = "left";
pub const SEEK_FORWARD: &str = "right";
pub const VOLUME_UP: &str = "up";
pub const VOLUME_DOWN: &str = "down";
pub const SET_RANGE_START: &str = "[";
pub const SET_RANGE_END: &str = "]";

#[cfg(target_os = "macos")]
pub const EXPORT: &str = "cmd-s";
#[cfg(not(target_os = "macos"))]
pub const EXPORT: &str = "ctrl-s";
