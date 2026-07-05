//! Application discovery: enumerate installed `.desktop` files, apply freedesktop
//! visibility rules, and resolve icons lazily (only for visible results, to keep
//! cold-start time minimal).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths, get_languages_from_env};

use crate::kde;

/// A launchable application distilled from a `.desktop` file.
#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    /// Stable desktop-file id (e.g. `org.kde.dolphin.desktop`), used for usage tracking.
    pub desktop_id: String,
    /// Path to the source `.desktop` file (used by `gio launch`).
    pub desktop_path: PathBuf,
    /// Raw `Icon=` value (name or absolute path), resolved to a file lazily.
    pub icon: Option<String>,
    /// Pre-expanded argv (field codes stripped) for the `systemd-run` fallback.
    pub argv: Vec<String>,
    pub terminal: bool,
}

/// Index all installed applications, best-effort. Higher-priority XDG directories
/// (e.g. `~/.local/share/applications`) shadow lower-priority ones by app-id.
pub fn index_apps() -> Vec<AppEntry> {
    let locales = get_languages_from_env();
    let desktops = current_desktops();

    let mut seen: HashSet<String> = HashSet::new();
    let mut apps: Vec<AppEntry> = Vec::new();

    for entry in Iter::new(default_paths()).entries(Some(&locales)) {
        // First occurrence of an app-id wins; a shadowing (even hidden) entry in a
        // higher-priority dir intentionally suppresses the lower-priority one.
        if !seen.insert(entry.appid.clone()) {
            continue;
        }
        if !is_visible(&entry, &desktops) {
            continue;
        }
        if let Some(app) = to_app_entry(&entry, &locales) {
            apps.push(app);
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Apply the desktop-entry-spec rules for what a launcher should show.
fn is_visible(e: &DesktopEntry, desktops: &[String]) -> bool {
    if e.type_() != Some("Application") {
        return false;
    }
    if e.no_display() || e.hidden() {
        return false;
    }
    // Must be launchable: either an Exec line, or D-Bus activation.
    if e.exec().is_none() && !e.dbus_activatable() {
        return false;
    }
    // TryExec gates on the named binary actually being installed.
    if let Some(try_exec) = e.try_exec() {
        if !binary_exists(try_exec) {
            return false;
        }
    }
    // OnlyShowIn / NotShowIn desktop-environment gating (names are case-sensitive).
    if let Some(only) = e.only_show_in() {
        if !only.iter().any(|d| desktops.iter().any(|c| c == d)) {
            return false;
        }
    }
    if let Some(not) = e.not_show_in() {
        if not.iter().any(|d| desktops.iter().any(|c| c == d)) {
            return false;
        }
    }
    true
}

fn to_app_entry(e: &DesktopEntry, locales: &[String]) -> Option<AppEntry> {
    let name = e.name(locales)?.to_string();
    if name.is_empty() {
        return None;
    }
    let argv = e.parse_exec_with_uris(&[], locales).unwrap_or_default();

    Some(AppEntry {
        name,
        generic_name: e
            .generic_name(locales)
            .map(|c| c.to_string())
            .filter(|s| !s.is_empty()),
        comment: e
            .comment(locales)
            .map(|c| c.to_string())
            .filter(|s| !s.is_empty()),
        keywords: e
            .keywords(locales)
            .map(|v| v.into_iter().map(|c| c.to_string()).collect())
            .unwrap_or_default(),
        desktop_id: e.appid.clone(),
        desktop_path: e.path.clone(),
        icon: e.icon().map(|s| s.to_string()),
        argv,
        terminal: e.terminal(),
    })
}

/// The current desktop(s) from `XDG_CURRENT_DESKTOP` (colon-separated, e.g. "KDE").
fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// True if `name` is an absolute existing path or is found on `$PATH`.
fn binary_exists(name: &str) -> bool {
    let p = Path::new(name);
    if p.is_absolute() {
        return p.exists();
    }
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(name).exists())
        })
        .unwrap_or(false)
}

// ---- icon resolution -------------------------------------------------------

/// Resolves freedesktop icon names to concrete file paths, memoized. The icon
/// theme comes from config or, by default, KDE's `kdeglobals`.
pub struct IconResolver {
    theme: String,
    size: u16,
    cache: HashMap<String, Option<PathBuf>>,
}

impl IconResolver {
    pub fn new(size: u16, theme_override: Option<String>) -> Self {
        let theme = theme_override
            .filter(|s| !s.is_empty())
            .or_else(kde::icon_theme)
            .unwrap_or_else(|| "breeze".to_string());
        Self {
            theme,
            size: size.max(8),
            cache: HashMap::new(),
        }
    }

    /// Resolve an `Icon=` value to a file path, caching the result.
    pub fn resolve(&mut self, icon: &str) -> Option<PathBuf> {
        if let Some(hit) = self.cache.get(icon) {
            return hit.clone();
        }
        let resolved = self.lookup(icon);
        self.cache.insert(icon.to_string(), resolved.clone());
        resolved
    }

    fn lookup(&self, icon: &str) -> Option<PathBuf> {
        // The spec allows Icon= to be an absolute path already.
        let p = Path::new(icon);
        if p.is_absolute() {
            return p.exists().then(|| p.to_path_buf());
        }
        freedesktop_icons::lookup(icon)
            .with_size(self.size)
            .with_scale(2)
            .with_theme(&self.theme)
            .with_cache()
            .find()
    }
}
