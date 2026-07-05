use std::path::PathBuf;
use std::sync::LazyLock;

use iced::Color;

fn kdeglobals() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("kdeglobals"))
}

static KDE_GLOBALS: LazyLock<Option<String>> =
    LazyLock::new(|| kdeglobals().and_then(|path| std::fs::read_to_string(path).ok()));

pub fn value(group: &str, key: &str) -> Option<String> {
    let text = KDE_GLOBALS.as_deref()?;
    let header = format!("[{group}]");
    let mut in_group = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == header;
        } else if in_group
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn parse_rgb(s: &str) -> Option<Color> {
    let parts: Vec<u8> = s
        .split(',')
        .map(str::trim)
        .filter_map(|p| p.parse().ok())
        .collect();
    match parts.as_slice() {
        [r, g, b] => Some(Color::from_rgb8(*r, *g, *b)),
        [r, g, b, a] => Some(Color::from_rgba8(*r, *g, *b, *a as f32 / 255.0)),
        _ => None,
    }
}

pub fn color(group: &str, key: &str) -> Option<Color> {
    parse_rgb(&value(group, key)?)
}

pub fn icon_theme() -> Option<String> {
    value("Icons", "Theme").filter(|s| !s.is_empty())
}

pub fn font_family() -> Option<String> {
    let raw = value("General", "font")?;
    let family = raw.split(',').next()?.trim().to_string();
    (!family.is_empty()).then_some(family)
}

pub fn accent() -> Option<Color> {
    color("General", "AccentColor")
}

pub fn single_click() -> bool {
    value("KDE", "SingleClick")
        .map(|v| !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}
