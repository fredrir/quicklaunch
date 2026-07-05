use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub window: Window,
    pub input: Input,
    pub placement: Placement,
    pub behavior: Behavior,
    pub theme: ThemeCfg,
    pub font: FontCfg,
    pub icons: IconsCfg,
    pub plugins: Vec<Plugin>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Plugin {
    pub name: String,
    pub command: Vec<String>,
    pub enabled: bool,
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: Vec::new(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Input {
    pub placeholder: String,
    pub height: f32,
    pub font_size: Option<f32>,
    pub padding_horizontal: f32,
    pub padding_vertical: f32,
    pub show_search_icon: bool,
    pub search_icon_size: f32,
    pub icon_spacing: f32,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            placeholder: "Search applications…".to_string(),
            height: 52.0,
            font_size: None,
            padding_horizontal: 18.0,
            padding_vertical: 14.0,
            show_search_icon: true,
            search_icon_size: 22.0,
            icon_spacing: 12.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HorizontalPlacement {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalPlacement {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultsPlacement {
    #[default]
    Below,
    Above,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Placement {
    pub horizontal: HorizontalPlacement,
    pub vertical: VerticalPlacement,
    pub x_offset: f32,
    pub y_offset: f32,
    pub margin: f32,
    pub results: ResultsPlacement,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            horizontal: HorizontalPlacement::Center,
            vertical: VerticalPlacement::Center,
            x_offset: 0.0,
            y_offset: 0.0,
            margin: 24.0,
            results: ResultsPlacement::Below,
        }
    }
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
    pub resident: bool,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            opening_animation: false,
            close_on_click_outside: true,
            close_on_focus_loss: true,
            frequency_ranking: true,
            single_click: None,
            resident: false,
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
    pub show: bool,
    pub size: u16,
    pub theme: Option<String>,
    pub spacing: f32,
    pub show_fallback: bool,
}

impl Default for IconsCfg {
    fn default() -> Self {
        Self {
            show: true,
            size: 40,
            theme: None,
            spacing: 14.0,
            show_fallback: true,
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
                eprintln!(
                    "quicklaunch: config error in {}: {e}; using defaults",
                    path.display()
                );
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn example_config_is_valid() {
        toml::from_str::<Config>(include_str!("../config.example.toml")).unwrap();
    }
}
