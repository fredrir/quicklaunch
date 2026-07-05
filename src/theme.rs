use std::collections::HashMap;
use std::path::PathBuf;

use iced::Color;

use crate::config::{ThemeCfg, ThemeSource};
use crate::kde;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub text: Color,
    pub muted: Color,
    pub placeholder: Color,
    pub accent: Color,
    pub selection: Color,
    pub hairline: Color,
    pub faint: Color,
}

mod def {
    use iced::Color;
    pub const BG: Color = Color::from_rgb(0.070, 0.070, 0.086);
    pub const TEXT: Color = Color::from_rgb(0.925, 0.925, 0.925);
    pub const MUTED: Color = Color::from_rgb(0.604, 0.627, 0.651);
    pub const ACCENT: Color = Color::from_rgb(0.239, 0.682, 0.914);
    pub const SELECTION: Color = Color::from_rgb(0.239, 0.682, 0.914);
}

impl Theme {
    pub fn resolve(cfg: &ThemeCfg) -> Theme {
        let mut bg = def::BG;
        let mut text = def::TEXT;
        let mut muted = def::MUTED;
        let mut accent = def::ACCENT;
        let mut selection = def::SELECTION;

        let use_config = matches!(cfg.source, ThemeSource::Auto | ThemeSource::Config);
        let use_kde = matches!(cfg.source, ThemeSource::Auto | ThemeSource::Kde);

        let mut applied = false;
        if use_config {
            if let Some(p) = Config::load(cfg.config_path.as_deref()) {
                if let Some(c) = p.role("view_bg") { bg = c; }
                if let Some(c) = p.role("foreground") { text = c; }
                if let Some(c) = p.role("inactive") { muted = c; }
                if let Some(c) = p.role("accent") { accent = c; }
                if let Some(c) = p.role("selection_bg") { selection = c; }
                applied = true;
            }
        }
        if use_kde && !applied {
            if let Some(c) = kde::color("Colors:View", "BackgroundNormal") { bg = c; }
            if let Some(c) = kde::color("Colors:View", "ForegroundNormal") { text = c; }
            if let Some(c) = kde::color("Colors:View", "ForegroundInactive") { muted = c; }
            if let Some(c) = kde::accent().or_else(|| kde::color("Colors:Selection", "BackgroundNormal")) {
                accent = c;
            }
            if let Some(c) = kde::color("Colors:Selection", "BackgroundNormal") { selection = c; }
        }

        if let Some(c) = cfg.background.as_deref().and_then(parse_hex) { bg = c; }
        if let Some(c) = cfg.text.as_deref().and_then(parse_hex) { text = c; }
        if let Some(c) = cfg.muted.as_deref().and_then(parse_hex) { muted = c; }
        if let Some(c) = cfg.accent.as_deref().and_then(parse_hex) { accent = c; }
        if let Some(c) = cfg.selection.as_deref().and_then(parse_hex) { selection = c; }

        let placeholder = cfg
            .placeholder
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or_else(|| with_alpha(muted, 0.75));

        Theme {
            bg,
            text,
            muted,
            placeholder,
            accent,
            selection,
            hairline: with_alpha(text, 0.07),
            faint: with_alpha(text, 0.06),
        }
    }
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

pub fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color::from_rgb8(r, g, b))
}

struct Config {
    colors: HashMap<String, String>,
    roles: HashMap<String, String>,
}

impl Config {
    fn load(path: Option<&str>) -> Option<Config> {
        let path = expand_tilde(path.unwrap_or("~/.config/quicklaunch/config.toml"));
        let text = std::fs::read_to_string(path).ok()?;
        let value: toml::Value = toml::from_str(&text).ok()?;
        Some(Config {
            colors: string_table(value.get("config")?),
            roles: string_table(value.get("kde")?),
        })
    }

    fn role(&self, role: &str) -> Option<Color> {
        let name = self.roles.get(role)?;
        parse_hex(self.colors.get(name)?)
    }
}

fn string_table(v: &toml::Value) -> HashMap<String, String> {
    v.as_table()
        .map(|t| {
            t.iter()
                .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}
