//! Launching the selected application, detached, so it survives the launcher
//! exiting and lands in its own systemd scope under `app.slice`.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use crate::apps::AppEntry;

/// Launch an application, spawn-and-forget. Prefers `gio launch` (full desktop-file
/// semantics + correct cgroup), falling back to `systemd-run --user --scope`.
pub fn launch(app: &AppEntry) -> std::io::Result<()> {
    if app.terminal {
        return launch_terminal(app);
    }

    // Primary: `gio launch <desktop-file>` — glib moves the child into its own
    // transient scope under app.slice, and handles DBusActivatable + startup
    // notification for us.
    if which("gio") {
        let spawned = null_io(Command::new("gio").arg("launch").arg(&app.desktop_path)).spawn();
        if spawned.is_ok() {
            return Ok(());
        }
    }

    launch_argv(&app.argv)
}

/// Launch a `Terminal=true` app inside the user's terminal emulator.
fn launch_terminal(app: &AppEntry) -> std::io::Result<()> {
    let mut argv: Vec<String> = if which("xdg-terminal-exec") {
        // The freedesktop terminal-exec spec: `xdg-terminal-exec <cmd> <args...>`.
        vec!["xdg-terminal-exec".to_string()]
    } else if let Ok(term) = std::env::var("TERMINAL") {
        vec![term, "-e".to_string()]
    } else if which("konsole") {
        vec!["konsole".to_string(), "-e".to_string()]
    } else {
        vec!["xterm".to_string(), "-e".to_string()]
    };
    argv.extend(app.argv.iter().cloned());
    launch_argv(&argv)
}

/// Fallback launch path: run `argv` in its own systemd scope, or detached in a new
/// process group if systemd-run is somehow unavailable.
fn launch_argv(argv: &[String]) -> std::io::Result<()> {
    if argv.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "no executable to launch",
        ));
    }

    if which("systemd-run") {
        return null_io(
            Command::new("systemd-run")
                .args(["--user", "--scope", "--slice=app.slice", "--quiet", "--"])
                .args(argv),
        )
        .spawn()
        .map(|_| ());
    }

    // Last resort: detach into a new process group so a group-kill of the launcher
    // doesn't take the app down with it.
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).process_group(0);
    null_io(&mut cmd).spawn().map(|_| ())
}

fn null_io(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
}

/// True if `name` is found on `$PATH`.
fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).exists()))
        .unwrap_or(false)
}
