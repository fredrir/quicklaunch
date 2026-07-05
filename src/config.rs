use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub window: Window,
    pub behavior: Behavior,
    pub theme: ThemeCfg,
    pub font: FontCfg,
    pub icons: IconsCfg,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Window {
    pub width: f32,
    pub max_results: usize,
    pub radius: f32,
    pub row_height: f32,
    pub opacity: f32,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            width: 640.0,
            max_results: 8,
            radius: 16.0,
            row_height: 52.0,
            opacity: 0.96,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Behavior {
    pub opening_animation: bool,
    pub close_on_click_outside: bool,
    pub close_on_focus_loss: bool,
    pub frequency_ranking: bool,
    pub single_click: Option<bool>,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            opening_animation: false,
            close_on_click_outside: true,
            close_on_focus_loss: true,
            frequency_ranking: true,
            single_click: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSource {
    #[default]
    Auto,
    Config,
    Kde,
    Custom,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ThemeCfg {
    pub source: ThemeSource,
    pub config_path: Option<String>,
    pub accent: Option<String>,
    pub background: Option<String>,
    pub text: Option<String>,
    pub muted: Option<String>,
    pub selection: Option<String>,
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FontCfg {
    pub family: Option<String>,
    pub size: Option<f32>,
}

impl FontCfg {
    pub fn size(&self) -> f32 {
        self.size.unwrap_or(20.0)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct IconsCfg {
    pub size: u16,
    pub theme: Option<String>,
}

impl Default for IconsCfg {
    fn default() -> Self {
        Self {
            size: 40,
            theme: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = dirs::config_dir().map(|p| p.join("quicklaunch/config.toml")) else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                eprintln!("quicklaunch: config error in {}: {e}; using defaults", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}
