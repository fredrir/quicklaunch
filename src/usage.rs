use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    apps: HashMap<String, Record>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Record {
    count: u32,
    last: u64,
}

impl Usage {
    pub fn load() -> Self {
        let Some(path) = data_path() else {
            return Self::default();
        };
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn record(&mut self, desktop_id: &str) {
        let now = now_unix();
        let entry = self.apps.entry(desktop_id.to_string()).or_insert(Record { count: 0, last: now });
        entry.count = entry.count.saturating_add(1);
        entry.last = now;
        self.save();
    }

    pub fn boost(&self, desktop_id: &str) -> u32 {
        let Some(r) = self.apps.get(desktop_id) else {
            return 0;
        };
        let freq = ((1.0 + r.count as f64).ln() * 25.0) as u32;
        let age_days = now_unix().saturating_sub(r.last) as f64 / 86_400.0;
        let recency = (40.0 * (-age_days / 14.0).exp()) as u32; // ~2 weeks
        freq + recency
    }

    fn save(&self) {
        let Some(path) = data_path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = toml::to_string(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn data_path() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("quicklaunch/usage.toml"))
}
