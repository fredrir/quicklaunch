//! Native KDE settings, read directly from `~/.config/kdeglobals`.
//!
//! A tiny in-process kconfig reader (no D-Bus, no extra crates) so the launcher
//! derives colors, fonts, icon theme, and interaction settings from the live KDE
//! configuration.

use std::path::PathBuf;

use iced::Color;

fn kdeglobals() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("kdeglobals"))
}

/// Read a raw value from `[group] key=...` in kdeglobals. Groups may be nested
/// names like `Colors:View` (written `[Colors:View]`).
pub fn value(group: &str, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(kdeglobals()?).ok()?;
    let header = format!("[{group}]");
    let mut in_group = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == header;
        } else if in_group {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Parse a KDE `R,G,B` (or `R,G,B,A`) color value.
fn parse_rgb(s: &str) -> Option<Color> {
    let parts: Vec<u8> = s.split(',').map(str::trim).filter_map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [r, g, b] => Some(Color::from_rgb8(*r, *g, *b)),
        [r, g, b, a] => Some(Color::from_rgba8(*r, *g, *b, *a as f32 / 255.0)),
        _ => None,
    }
}

/// A color from a kdeglobals `[Colors:*]` group, e.g. `color("Colors:View", "BackgroundNormal")`.
pub fn color(group: &str, key: &str) -> Option<Color> {
    parse_rgb(&value(group, key)?)
}

/// The configured icon theme (`[Icons] Theme`), e.g. "Breeze Chameleon Dark".
pub fn icon_theme() -> Option<String> {
    value("Icons", "Theme").filter(|s| !s.is_empty())
}

/// The general UI font family (`[General] font` is `Family,size,...`).
pub fn font_family() -> Option<String> {
    let raw = value("General", "font")?;
    let family = raw.split(',').next()?.trim().to_string();
    (!family.is_empty()).then_some(family)
}

/// KDE's accent color (`[General] AccentColor`), if set.
pub fn accent() -> Option<Color> {
    color("General", "AccentColor")
}

/// Whether KDE is in single-click mode (`[KDE] SingleClick`, default true).
pub fn single_click() -> bool {
    value("KDE", "SingleClick")
        .map(|v| !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}
