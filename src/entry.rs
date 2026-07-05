use std::path::PathBuf;

use nucleo_matcher::Utf32String;

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub action: LaunchAction,
    pub search_name: Utf32String,
    pub search_generic_name: Option<Utf32String>,
    pub search_keywords: Vec<Utf32String>,
}

#[derive(Debug, Clone)]
pub enum LaunchAction {
    Desktop {
        path: PathBuf,
        argv: Vec<String>,
        terminal: bool,
        dbus_activatable: bool,
        working_dir: Option<PathBuf>,
    },
    Command {
        argv: Vec<String>,
        terminal: bool,
    },
}

impl Entry {
    pub fn new(
        id: String,
        name: String,
        generic_name: Option<String>,
        comment: Option<String>,
        keywords: Vec<String>,
        icon: Option<String>,
        action: LaunchAction,
    ) -> Self {
        let search_name = Utf32String::from(name.as_str());
        let search_generic_name = generic_name.as_deref().map(Utf32String::from);
        let search_keywords = keywords
            .iter()
            .map(|keyword| Utf32String::from(keyword.as_str()))
            .collect();
        Self {
            id,
            name,
            generic_name,
            comment,
            icon,
            icon_path: None,
            action,
            search_name,
            search_generic_name,
            search_keywords,
        }
    }

    pub fn terminal(&self) -> bool {
        match &self.action {
            LaunchAction::Desktop { terminal, .. } | LaunchAction::Command { terminal, .. } => {
                *terminal
            }
        }
    }
}
