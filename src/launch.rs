use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use crate::entry::{Entry, LaunchAction};

pub fn launch(entry: &Entry) -> std::io::Result<()> {
    match &entry.action {
        LaunchAction::Desktop { path, argv, terminal } => {
            if *terminal {
                return launch_terminal(argv);
            }
            if which("gio") {
                let spawned = null_io(Command::new("gio").arg("launch").arg(path)).spawn();
                if spawned.is_ok() {
                    return Ok(());
                }
            }
            launch_argv(argv)
        }
        LaunchAction::Command { argv, terminal } => {
            if *terminal {
                launch_terminal(argv)
            } else {
                launch_argv(argv)
            }
        }
    }
}

fn launch_terminal(command: &[String]) -> std::io::Result<()> {
    let mut argv: Vec<String> = if which("xdg-terminal-exec") {
        vec!["xdg-terminal-exec".to_string()]
    } else if let Ok(term) = std::env::var("TERMINAL") {
        vec![term, "-e".to_string()]
    } else if which("konsole") {
        vec!["konsole".to_string(), "-e".to_string()]
    } else {
        vec!["xterm".to_string(), "-e".to_string()]
    };
    argv.extend(command.iter().cloned());
    launch_argv(&argv)
}

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

    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).process_group(0);
    null_io(&mut cmd).spawn().map(|_| ())
}

fn null_io(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
}

fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).exists()))
        .unwrap_or(false)
}
