use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    pub icon: Option<String>,
    pub action: LaunchAction,
}

#[derive(Debug, Clone)]
pub enum LaunchAction {
    Desktop {
        path: PathBuf,
        argv: Vec<String>,
        terminal: bool,
    },
    Command {
        argv: Vec<String>,
        terminal: bool,
    },
}

impl Entry {
    pub fn terminal(&self) -> bool {
        match &self.action {
            LaunchAction::Desktop { terminal, .. } | LaunchAction::Command { terminal, .. } => {
                *terminal
            }
        }
    }
}
