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

struct Applications;

impl Provider for Applications {
    fn name(&self) -> &str {
        "applications"
    }

    fn load(&self) -> Result<Vec<Entry>, String> {
        Ok(apps::index_apps())
    }
}

struct External {
    config: Plugin,
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

        entries
            .into_iter()
            .map(|entry| {
                if entry.id.trim().is_empty() || entry.name.trim().is_empty() {
                    return Err("entry id and name must not be empty".to_string());
                }
                if entry.command.is_empty() {
                    return Err(format!("entry {:?} has an empty command", entry.id));
                }

                Ok(Entry {
                    id: format!("{}:{}", self.config.name, entry.id),
                    name: entry.name,
                    generic_name: entry.generic_name,
                    comment: entry.comment,
                    keywords: entry.keywords,
                    icon: entry.icon,
                    action: LaunchAction::Command {
                        argv: entry.command,
                        terminal: entry.terminal,
                    },
                })
            })
            .collect()
    }
}

pub fn load(plugins: &[Plugin]) -> Vec<Entry> {
    let mut providers: Vec<Box<dyn Provider>> = vec![Box::new(Applications)];
    providers.extend(
        plugins
            .iter()
            .filter(|plugin| plugin.enabled)
            .map(|config| Box::new(External { config: config.clone() }) as Box<dyn Provider>),
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

pub fn load_applications() -> Vec<Entry> {
    Applications.load().unwrap_or_default()
}

pub fn load_plugin(config: Plugin) -> Vec<Entry> {
    let provider = External { config };
    match provider.load() {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("quicklaunch: provider {}: {error}", provider.name());
            Vec::new()
        }
    }
}
