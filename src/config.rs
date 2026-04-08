use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    let mut p = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    p.push("annotations");
    p.push("config.toml");
    p
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DrawingConfig {
    /// "permanent" or "fade" (fade not implemented in MVP, kept for future)
    pub stroke_persistence: String,
}

impl Default for DrawingConfig {
    fn default() -> Self {
        Self { stroke_persistence: "permanent".to_string() }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolbarConfig {
    /// [x, y] position from top-left of screen
    pub position: [i32; 2],
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self { position: [40, 40] }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub drawing: DrawingConfig,
    #[serde(default)]
    pub toolbar: ToolbarConfig,
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("annotations: failed to parse config, using defaults: {e}");
                Config::default()
            })
        } else {
            Config::default()
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("annotations: failed to create config dir: {e}");
                return;
            }
        }
        match toml::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = fs::write(&path, content) {
                    eprintln!("annotations: failed to write config: {e}");
                }
            }
            Err(e) => eprintln!("annotations: failed to serialize config: {e}"),
        }
    }
}
