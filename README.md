# kde-app-launcher

A minimal, fast, Spotlight-style application launcher for **KDE Plasma 6 / KWin on
Wayland**, written in Rust. It shows a centered, translucent search bar as a
`wlr-layer-shell` overlay; results appear only once you start typing.

- **Rendering:** [`iced`](https://github.com/iced-rs/iced) (0.14) on wgpu/Vulkan — GPU-accelerated, pure Rust.
- **Overlay:** [`iced_layershell`](https://crates.io/crates/iced_layershell) (0.18) — a real Wayland layer surface.
- **App discovery:** `freedesktop-desktop-entry` — parses `.desktop` files with correct visibility rules.
- **Search:** `nucleo-matcher` (Helix's fuzzy engine), smart-case.
- **Icons:** `freedesktop-icons` — resolves your KDE icon theme (from `kdeglobals`).
- **Launching:** `gio launch` (apps land in their own `app.slice` systemd scope and
  survive the launcher exiting), falling back to `systemd-run --user --scope`.

## Build

```sh
cargo build --release
# binary: target/release/kde-app-launcher
```

## Run

```sh
./target/release/kde-app-launcher
```

- Type to filter. Results appear below the bar.
- **↑ / ↓** move the selection, **Enter** launches, **Esc** dismisses.
- **Click** a result to launch it.
- Launching once, then pressing the hotkey again while it's open, dismisses it
  (single-instance toggle).

### Headless check (no GUI)

```sh
./target/release/kde-app-launcher --list          # how many apps are indexed + a sample
./target/release/kde-app-launcher --list firefox  # ranked matches for a query
```

## Bind it to a global shortcut (e.g. Meta+Space)

> Note: on a default Plasma setup **Meta+Space is KRunner**. Rebinding it to this
> launcher takes that key away from KRunner.

**Via System Settings (reliable):**
System Settings → Keyboard → Shortcuts → **Add New → Command or Script** →
set the command to the absolute path of `target/release/kde-app-launcher` →
click the shortcut field and press your combo (e.g. Meta+Space). If Meta+Space is
taken, KDE will warn and offer to reassign it.

## Design

Centered "spotlight": a rounded, translucent search pill, growing a results panel
below with app icons + name + a muted subtitle, accent-highlighted selection.
All visual constants live in [`src/style.rs`](src/style.rs) — tune colors, radii,
sizes, and the top offset there. (KWin gives per-pixel transparency to layer
surfaces but no blur-behind, so the panel background is kept fairly opaque.)

## Architecture

| File | Responsibility |
|------|----------------|
| `src/main.rs`   | Entry point; single-instance toggle; `--list` debug path. |
| `src/ui.rs`     | The iced layer-shell app: state, update, view, keyboard handling. |
| `src/style.rs`  | Design tokens and component styles. |
| `src/apps.rs`   | `.desktop` discovery, freedesktop visibility filtering, lazy icon resolution. |
| `src/search.rs` | nucleo fuzzy ranking. |
| `src/launch.rs` | Detached launching into a systemd scope; terminal-app handling. |
| `src/single.rs` | Single-instance / toggle-on-second-press via a pid file. |

## Performance

- Spawn-per-invoke: no resident daemon, zero idle memory. Cold start is fast on
  modern hardware (wgpu/Vulkan device init + indexing ~60 apps).
- Icons are resolved **lazily** — only for the handful of visible results — so
  startup does no icon-theme I/O for the full app list.
- Release profile uses `lto=thin`, `codegen-units=1`, `panic=abort`, `strip`.

If cold start ever feels slow, the next step is a **resident daemon** that keeps the
index warm and toggles the surface on an IPC ping; `apps`/`search`/`launch` are
written to drop straight into that model.

## Notes / limitations

- Wayland only (no X11 fallback — `iced_layershell` is SCTK-based). Requires
  `WAYLAND_DISPLAY` to be set.
- The overlay takes a keyboard grab (`KeyboardInteractivity::Exclusive`) so typing
  works immediately. Esc always dismisses it.
