# quicklaunch

`io.github.fredrir.quicklaunch` — a minimal, fast, Spotlight-style application launcher
for **KDE Plasma 6 / KWin on Wayland**, written in Rust. It shows a centered, translucent
search bar as a `wlr-layer-shell` overlay; results appear only once you start typing.

- **Rendering:** [`iced`](https://github.com/iced-rs/iced) (0.14) on wgpu/Vulkan — GPU-accelerated, pure Rust.
- **Overlay:** [`iced_layershell`](https://crates.io/crates/iced_layershell) (0.18) — a real Wayland layer surface.
- **App discovery:** `freedesktop-desktop-entry` — parses `.desktop` files with correct visibility rules.
- **Search:** `nucleo-matcher` (Helix's fuzzy engine), smart-case, with frequency/recency ranking.
- **Theme & icons:** derived from `~/dotfiles/theme/palette.toml` → the live KDE color scheme → defaults.
- **Launching:** `gio launch` (apps land in their own `app.slice` systemd scope and
  survive the launcher exiting), falling back to `systemd-run --user --scope`.

## Build

```sh
cargo build --release
# binary: target/release/quicklaunch
```

## Run

```sh
./target/release/quicklaunch
```

- Type to filter. Results appear below the bar.
- Move selection: **↑/↓**, **Tab / Shift+Tab**, **Ctrl+N/P**, **Ctrl+J/K**, **PageUp/PageDown**.
- **Enter** launches the selection. **Esc**, **clicking outside**, or switching focus away dismisses.
- **Click** a result to launch (or select-then-click in KDE double-click mode).
- Pressing the hotkey again while open dismisses it (single-instance toggle).

### Headless checks (no GUI)

```sh
quicklaunch --list          # how many apps are indexed + a sample
quicklaunch --list firefox  # ranked matches (incl. usage boost) for a query
quicklaunch --theme         # print the resolved theme colors
```

## Configuration

Optional, at `~/.config/quicklaunch/config.toml`. Every key is optional and the file is
**re-read on every open** (no daemon/restart). See [`config.example.toml`](config.example.toml)
for the full annotated list. Highlights:

```toml
[window]   width=640  top_offset=220  max_results=8  radius=16  row_height=52  opacity=0.96
[behavior] close_on_click_outside=true  close_on_focus_loss=true  frequency_ranking=true
           # single_click: omit to follow KDE, or set true/false
[theme]    source="auto"   # auto | palette | kde | custom
           palette_path="~/dotfiles/theme/palette.toml"
           # accent/background/text/muted/selection/placeholder = "#rrggbb" (override any)
[font]     # family: omit to follow KDE, else "Noto Sans";  size=20
[icons]    size=40  # theme: omit to follow KDE
```

## KDE integration

- **Colors** — resolved from `palette.toml`'s `[kde]` roles (`view_bg`, `foreground`,
  `inactive`, `accent`, `selection_bg`) mapped through `[palette]`; falls back to the live
  KDE color scheme (`kdeglobals [Colors:*]`), then built-in defaults. `[theme].source` and
  the per-field hex overrides let you pin any of this.
- **Icons** — resolved against your KDE icon theme (`kdeglobals [Icons] Theme`).
- **Font** — follows KDE's general font (`kdeglobals [General] font`) unless overridden.
- **Single-click** — click-to-launch vs select-then-launch follows KDE's `[KDE] SingleClick`.
- **Frequency/recency** — launches are tracked in `~/.local/share/quicklaunch/usage.toml`;
  frequently/recently used apps are boosted in the ranking (results still only appear once
  you type — this only reorders matches).
- **Animations** — intentionally none: the launcher appears instantly (a snappy launcher is
  the goal; this respects users who lower KDE's animation factor).

## Bind to Meta+Space

A `NoDisplay` desktop entry `io.github.fredrir.quicklaunch.desktop` (in
`~/.local/share/applications/`) plus a `[services]` entry in `kglobalshortcutsrc` binds the
launcher to **Meta+Space** (KRunner keeps its XF86Search key).

> **Activation:** on Plasma 6 Wayland, `org.kde.kglobalaccel` is hosted by `kwin_wayland`
> itself, and it only registers a **brand-new** command shortcut at session start. So after
> editing `kglobalshortcutsrc`, **log out and back in** to activate it (restarting
> `plasma-kglobalaccel.service` is a no-op — it doesn't own the service). Alternatively,
> binding it through **System Settings → Shortcuts → Add New → Command or Script** registers
> it live via the KGlobalAccel D-Bus API.

## Architecture

| File | Responsibility |
|------|----------------|
| `src/main.rs`   | Entry point; `--list`/`--theme`/`--query` debug paths; single-instance toggle. |
| `src/ui.rs`     | The iced layer-shell app: state, update, view, keyboard, click-outside. |
| `src/config.rs` | `config.toml` schema + loading. |
| `src/theme.rs`  | Color resolution (palette → KDE → defaults + overrides). |
| `src/kde.rs`    | In-process `kdeglobals` reader (colors, font, icon theme, single-click). |
| `src/style.rs`  | Theme-driven component styles + fixed geometry. |
| `src/apps.rs`   | `.desktop` discovery, freedesktop visibility filtering, lazy icon resolution. |
| `src/search.rs` | nucleo fuzzy ranking + usage boost. |
| `src/usage.rs`  | Frequency/recency tracking (`usage.toml`). |
| `src/launch.rs` | Detached launching into a systemd scope; terminal-app handling. |
| `src/single.rs` | Single-instance / toggle-on-second-press via a pid file. |

## Performance

Spawn-per-invoke (no resident daemon, zero idle memory). Process start + full app
indexing ≈ 3 ms; icons resolved lazily for only the visible rows; release profile uses
`lto=thin`, `codegen-units=1`, `panic=abort`, `strip`. If cold start ever feels slow, the
`apps`/`search`/`launch`/`theme` modules are written to drop into a resident-daemon mode.

## Notes / limitations

- Wayland only (no X11 fallback — `iced_layershell` is SCTK-based).
- The overlay takes a keyboard grab so typing works immediately; Esc always dismisses it.
- KWin gives layer surfaces per-pixel transparency but no blur-behind, so the panel is
  kept fairly opaque (`[window].opacity`).
