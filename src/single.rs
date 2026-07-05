//! Single-instance handling with toggle-on-second-press semantics.
//!
//! The launcher is spawned fresh on every Meta+Space press. If an instance is
//! already showing, a second press should dismiss it — so we detect the running
//! instance via a pid file and send it SIGTERM instead of stacking a new overlay.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Substring expected in the running process's cmdline, to guard against pid reuse.
const BIN_MARKER: &str = "kde-app-launcher";

fn pid_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("kde-app-launcher.pid")
}

/// If another launcher instance is running, signal it to quit and return `true`
/// (this press toggles the launcher closed — the caller should exit). Otherwise
/// record our pid and return `false` (the caller should show the UI).
pub fn toggle_or_register() -> bool {
    let path = pid_path();

    if let Ok(contents) = fs::read_to_string(&path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            if pid != std::process::id() && is_running_instance(pid) {
                let _ = Command::new("kill").arg(pid.to_string()).status();
                return true;
            }
        }
    }

    let _ = fs::write(&path, std::process::id().to_string());
    false
}

/// Remove our pid file. Called on a clean exit; a SIGTERM'd instance leaves a
/// stale file, which the next launch self-heals (dead pid fails the liveness check).
pub fn cleanup() {
    let _ = fs::remove_file(pid_path());
}

/// True if `pid` is alive and is actually one of our launcher processes.
fn is_running_instance(pid: u32) -> bool {
    // /proc/<pid>/cmdline is NUL-separated argv; a dead pid yields an empty read.
    match fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(bytes) if !bytes.is_empty() => {
            String::from_utf8_lossy(&bytes).contains(BIN_MARKER)
        }
        _ => false,
    }
}
