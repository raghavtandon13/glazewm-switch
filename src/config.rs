use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    Classic,
    Windows,
}

impl Default for Style {
    fn default() -> Self {
        Style::Windows
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub position: Position,
    #[serde(default)]
    pub style: Style,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Default for Position {
    fn default() -> Self {
        Self { x: 48, y: 0 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            position: Position::default(),
            style: Style::default(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("glazewm-switch.toml");

        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => {
                        log::info!("Loaded config from {:?}", config_path);
                        return config;
                    }
                    Err(e) => {
                        log::warn!("Failed to parse config: {}, using defaults", e);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read config: {}, using defaults", e);
                }
            }
        }

        log::info!("Using default config");
        Self::default()
    }
}
