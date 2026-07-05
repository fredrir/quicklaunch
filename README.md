# quicklaunch

`io.github.fredrir.quicklaunch`
Application launcher for KDE Plasma / KWin on Wayland

## Build

```sh
cargo build --release
```

## Run

```sh
./target/release/quicklaunch
```

### Headless checks

```sh
quicklaunch --list          # num of matches
quicklaunch --list firefox  # ranked matches
quicklaunch --theme         # theme colors
```

## Configuration

See [`config.example.toml`](config.example.toml) for full example.

## Plugins

Configured with `[[plugins]]`:

```toml
[[plugins]]
name = "bookmarks"
command = ["python3", "/path/to/bookmarks-plugin.py"]
enabled = true
```
