use std::fs;
use std::path::PathBuf;
use std::process::Command;

const BIN_MARKER: &str = "quicklaunch";

fn pid_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("quicklaunch.pid")
}

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

pub fn cleanup() {
    let _ = fs::remove_file(pid_path());
}

fn is_running_instance(pid: u32) -> bool {
    match fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(bytes) if !bytes.is_empty() => {
            String::from_utf8_lossy(&bytes).contains(BIN_MARKER)
        }
        _ => false,
    }
}
