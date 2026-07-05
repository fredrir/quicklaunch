use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::apps;
use crate::config::Plugin;
use crate::entry::{Entry, LaunchAction};

pub const PROTOCOL_VERSION: &str = "1";

pub trait Provider {
    fn name(&self) -> &str;
    fn load(&self) -> Result<Vec<Entry>, String>;
}

struct Applications {
    icon_size: u16,
    icon_theme: Option<String>,
}

impl Provider for Applications {
    fn name(&self) -> &str {
        "applications"
    }

    fn load(&self) -> Result<Vec<Entry>, String> {
        Ok(apps::index_apps(self.icon_size, self.icon_theme.clone()))
    }
}

struct External {
    config: Plugin,
    icon_size: u16,
    icon_theme: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExternalEntry {
    id: String,
    name: String,
    #[serde(default)]
    generic_name: Option<String>,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    icon: Option<String>,
    command: Vec<String>,
    #[serde(default)]
    terminal: bool,
}

impl Provider for External {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn load(&self) -> Result<Vec<Entry>, String> {
        let Some(program) = self.config.command.first() else {
            return Err("command is empty".to_string());
        };
        if self.config.name.trim().is_empty() {
            return Err("name is empty".to_string());
        }

        let output = Command::new(program)
            .args(&self.config.command[1..])
            .env("QUICKLAUNCH_PLUGIN_PROTOCOL", PROTOCOL_VERSION)
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|error| format!("could not run {program}: {error}"))?;

        if !output.status.success() {
            return Err(format!("exited with {}", output.status));
        }

        let entries: Vec<ExternalEntry> = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("invalid JSON: {error}"))?;

        let mut entries: Vec<Entry> = entries
            .into_iter()
            .map(|entry| {
                if entry.id.trim().is_empty() || entry.name.trim().is_empty() {
                    return Err("entry id and name must not be empty".to_string());
                }
                if entry.command.is_empty() {
                    return Err(format!("entry {:?} has an empty command", entry.id));
                }

                let action = LaunchAction::Command {
                    argv: entry.command,
                    terminal: entry.terminal,
                };
                Ok(Entry::new(
                    format!("{}:{}", self.config.name, entry.id),
                    entry.name,
                    entry.generic_name,
                    entry.comment,
                    entry.keywords,
                    entry.icon,
                    action,
                ))
            })
            .collect::<Result<_, _>>()?;
        apps::resolve_icons(&mut entries, self.icon_size, self.icon_theme.clone());
        Ok(entries)
    }
}

pub fn load(plugins: &[Plugin], icon_size: u16, icon_theme: Option<String>) -> Vec<Entry> {
    let mut providers: Vec<Box<dyn Provider>> = vec![Box::new(Applications {
        icon_size,
        icon_theme: icon_theme.clone(),
    })];
    providers.extend(
        plugins
            .iter()
            .filter(|plugin| plugin.enabled)
            .map(|config| {
                Box::new(External {
                    config: config.clone(),
                    icon_size,
                    icon_theme: icon_theme.clone(),
                }) as Box<dyn Provider>
            }),
    );

    let mut entries = Vec::new();
    for provider in providers {
        match provider.load() {
            Ok(mut loaded) => entries.append(&mut loaded),
            Err(error) => eprintln!("quicklaunch: provider {}: {error}", provider.name()),
        }
    }
    entries
}

pub fn load_applications(icon_size: u16, icon_theme: Option<String>) -> Vec<Entry> {
    Applications {
        icon_size,
        icon_theme,
    }
    .load()
    .unwrap_or_default()
}

pub fn load_plugin(config: Plugin, icon_size: u16, icon_theme: Option<String>) -> Vec<Entry> {
    let provider = External {
        config,
        icon_size,
        icon_theme,
    };
    match provider.load() {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("quicklaunch: provider {}: {error}", provider.name());
            Vec::new()
        }
    }
}
