use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use freedesktop_desktop_entry::{default_paths, get_languages_from_env, DesktopEntry, Iter};

use crate::entry::{Entry, LaunchAction};
use crate::kde;

pub fn index_apps() -> Vec<Entry> {
    let locales = get_languages_from_env();
    let desktops = current_desktops();

    let mut seen: HashSet<String> = HashSet::new();
    let mut apps: Vec<Entry> = Vec::new();

    for entry in Iter::new(default_paths()).entries(Some(&locales)) {
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

fn is_visible(e: &DesktopEntry, desktops: &[String]) -> bool {
    if e.type_() != Some("Application") {
        return false;
    }
    if e.no_display() || e.hidden() {
        return false;
    }
    if e.exec().is_none() && !e.dbus_activatable() {
        return false;
    }
    if let Some(try_exec) = e.try_exec() {
        if !binary_exists(try_exec) {
            return false;
        }
    }
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

fn to_app_entry(e: &DesktopEntry, locales: &[String]) -> Option<Entry> {
    let name = e.name(locales)?.to_string();
    if name.is_empty() {
        return None;
    }
    let argv = e.parse_exec_with_uris(&[], locales).unwrap_or_default();

    Some(Entry {
        id: e.appid.clone(),
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
        icon: e.icon().map(|s| s.to_string()),
        action: LaunchAction::Desktop {
            path: e.path.clone(),
            argv,
            terminal: e.terminal(),
        },
    })
}

fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

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

    pub fn resolve(&mut self, icon: &str) -> Option<PathBuf> {
        if let Some(hit) = self.cache.get(icon) {
            return hit.clone();
        }
        let resolved = self.lookup(icon);
        self.cache.insert(icon.to_string(), resolved.clone());
        resolved
    }

    fn lookup(&self, icon: &str) -> Option<PathBuf> {
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
