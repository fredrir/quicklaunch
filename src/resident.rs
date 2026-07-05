use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use iced_futures::futures::SinkExt;

static SOCKET: OnceLock<Arc<UnixDatagram>> = OnceLock::new();

fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("quicklaunch.sock")
}

pub fn notify(query: Option<&str>) -> bool {
    let Ok(socket) = UnixDatagram::unbound() else {
        return false;
    };
    let payload = query.unwrap_or_default().as_bytes();
    socket.send_to(payload, socket_path()).is_ok()
}

pub fn claim() -> std::io::Result<()> {
    let path = socket_path();
    let _ = fs::remove_file(&path);
    let socket = UnixDatagram::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    SOCKET
        .set(Arc::new(socket))
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::AlreadyExists, "resident socket"))
}

pub fn subscription() -> iced::Subscription<Option<String>> {
    iced::Subscription::run(events)
}

fn events() -> impl iced_futures::futures::Stream<Item = Option<String>> {
    iced_futures::stream::channel(8, async |mut output| {
        let Some(socket) = SOCKET.get().cloned() else {
            return;
        };
        let _ = std::thread::Builder::new()
            .name("quicklaunch-ipc".to_string())
            .spawn(move || {
                let mut buffer = vec![0_u8; 64 * 1024];
                while let Ok(size) = socket.recv(&mut buffer) {
                    let query = if size == 0 {
                        None
                    } else {
                        Some(String::from_utf8_lossy(&buffer[..size]).into_owned())
                    };
                    if iced_futures::futures::executor::block_on(output.send(query)).is_err() {
                        break;
                    }
                }
            });
        iced_futures::futures::future::pending::<()>().await;
    })
}

pub fn cleanup() {
    let _ = fs::remove_file(socket_path());
}
