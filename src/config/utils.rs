use std::{env, fs, path::PathBuf};

use serde::{Serialize, de::DeserializeOwned};

/// Base directory shared by all settings files
pub fn base_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let base = env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("APPDATA is not set"))?;
        Ok(base.join("fast_clip"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
        Ok(home
            .join("Library")
            .join("Application Support")
            .join("fast_clip"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(path).join("fast_clip"));
        }
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
        Ok(home.join(".config").join("fast_clip"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        Ok(env::current_dir()?.join("fast_clip"))
    }
}

/// Load a JSON settings file from the base directory.
/// Missing or unparseable files silently fall back to `T::default()`.
pub fn load_json<T: Default + DeserializeOwned>(file_name: &str) -> T {
    let Ok(path) = base_dir().map(|dir| dir.join(file_name)) else {
        return T::default();
    };
    println!("Read {} from {:?}", file_name, path);

    let Ok(content) = fs::read_to_string(path) else {
        return T::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Save a JSON settings file into the base directory, creating it if needed.
pub fn save_json<T: Serialize>(file_name: &str, value: &T) -> anyhow::Result<()> {
    let path = base_dir()?.join(file_name);
    println!("Save {} to {:?}", file_name, path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(value)?;
    fs::write(path, content)?;
    Ok(())
}
