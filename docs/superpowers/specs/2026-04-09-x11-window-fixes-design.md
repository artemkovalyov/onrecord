# Annotations — X11 Window Fixes
**Date:** 2026-04-09  
**Status:** Approved  
**Stack:** Rust + GTK4 + Cairo + gdk4-x11 + x11rb

---

## Overview

Fix two blocking issues on X11/KDE Plasma (KWin):

1. **Overlay transparency** — overlay renders as an opaque dark window instead of transparent, and does not cover the full screen
2. **Always-on-top** — toolbar gets covered by other windows; GTK4 removed `set_keep_above()` so no keep-above hint is being set

Both fixes use `gdk4-x11` to get X11 handles from GDK objects and `x11rb` to set X11 window properties. All platform-specific code is isolated in a new `src/platform.rs` module with a Wayland stub for future extension.

---

## Architecture

### New module: `src/platform.rs`

All X11/Wayland-specific window setup lives here. The rest of the codebase calls three functions:

```rust
pub fn setup_overlay(window: &ApplicationWindow, monitor_geometry: gdk4::Rectangle)
pub fn setup_toolbar(window: &ApplicationWindow, position: [i32; 2])
pub fn get_window_position(window: &ApplicationWindow) -> Option<[i32; 2]>
```

Internally, `platform.rs` detects the session type at runtime:

```rust
fn is_x11() -> bool {
    gdk4::Display::default()
        .and_then(|d| d.downcast::<gdk4_x11::X11Display>().ok())
        .is_some()
}
```

- If X11: apply X11 property fixes via `x11rb`
- If Wayland: log a warning, do nothing (stub for future `gtk4-layer-shell` support)
- If neither: silent no-op

### Detection strategy

Runtime detection via GDK display downcast. No compile-time feature flags needed. The `XDG_SESSION_TYPE` env var is a valid fallback but the GDK downcast is more reliable.

---

## Overlay Fix

### Problem

`window.fullscreen()` hands control to KWin, which applies its own compositing pipeline. KWin does not honor the RGBA visual request for fullscreened windows and renders them opaque. The window also doesn't cover the full monitor (compositor adds insets).

### Solution

Replace `window.fullscreen()` with manual geometry + X11 property setup:

1. **Request RGBA visual** before the window is mapped. Use `gdk4_x11::X11Screen::rgba_visual()` to get an RGBA-capable visual and apply it to the window surface before it is realized.

2. **Set `_NET_WM_WINDOW_TYPE_DOCK`** — tells KWin this is a panel/overlay. KWin skips WM decoration and standard compositing rules for dock-type windows, which allows RGBA transparency to work correctly.

3. **Set `_NET_WM_STATE_ABOVE`** via `x11rb` `ChangeProperty` after the window is mapped (requires XID from `gdk4_x11::X11Surface::xid()`). `_NET_WM_STATE_FULLSCREEN` is not used — `DOCK` type windows don't need it, and combining them can confuse KWin.

4. **Manual geometry** — read monitor size from `gdk4::Display::monitors()` (primary monitor), call `window.set_default_size(w, h)` and position via `x11rb` `ConfigureWindow` to `(0, 0)`. This replaces `window.fullscreen()` entirely.

### Why not `_NET_WM_WINDOW_TYPE_SPLASH`?

`DOCK` is the correct type: it means "this window should be above other windows and is not managed by the WM." `SPLASH` is for loading screens and has different compositor behavior.

### Key call sequence

```
build_overlay()
  → window.connect_realize(|w| platform::setup_overlay(w, monitor_rect))
  → remove window.fullscreen() call
```

`setup_overlay` runs inside `connect_realize` because the X11 XID is only available after the surface is created.

---

## Toolbar Fix

### Problem

No always-on-top hint is set. KWin treats the toolbar as a normal window and other windows freely cover it. Toolbar position loads from config but is never applied (the `let [_tx, _ty]` code is dead).

### Solution

1. **Set `_NET_WM_STATE_ABOVE`** via `x11rb` after window is mapped — toolbar floats above all normal windows including fullscreen apps.

2. **Set `_NET_WM_STATE_STICKY`** — toolbar appears on all virtual desktops.

3. **Apply position from config** via `x11rb` `ConfigureWindow` with `(x, y)` from `cfg.toolbar.position`. Runs in `connect_realize`.

4. **Save position on exit** — `app.connect_shutdown` reads toolbar XID geometry via `x11rb` `GetGeometry`, writes `[x, y]` to config via `Config::save()`.

### Key call sequence

```
build_toolbar()
  → window.connect_realize(|w| platform::setup_toolbar(w, cfg.toolbar.position))

main() app.connect_shutdown
  → position = platform::get_window_position(&toolbar_win)
  → cfg.toolbar.position = position
  → cfg.save()
```

---

## New Dependencies

```toml
gdk4-x11 = "0.11"  # GDK X11 display/surface/screen types; XID access (matches gdk4 = "0.11")
x11rb = { version = "0.13", features = ["allow-unsafe-code"] }
```

`x11rb` requires `allow-unsafe-code` only for the connection setup (`x11rb::connect()`); the property-setting API itself is safe.

---

## Files Changed

| File | Change |
|------|--------|
| `src/platform.rs` | **New.** All X11 setup logic. |
| `src/main.rs` | Add `mod platform`. Wire `connect_shutdown` for position save. Pass monitor geometry to `build_overlay`. |
| `src/overlay.rs` | Remove `window.fullscreen()`. Add `connect_realize` hook calling `platform::setup_overlay`. |
| `src/toolbar.rs` | Add `connect_realize` hook calling `platform::setup_toolbar`. |
| `Cargo.toml` | Add `gdk4-x11`, `x11rb` dependencies. |

`state.rs`, `input.rs`, `stroke.rs`, `config.rs` — **untouched**.

---

## Wayland Path (Future)

When Wayland support is added, `platform.rs` gains a second branch:

```rust
if let Ok(wayland_display) = display.downcast::<gdk4_wayland::WaylandDisplay>() {
    // gtk4-layer-shell: set layer OVERLAY, anchor all edges, exclusive zone -1
}
```

Only `platform.rs` and `Cargo.toml` change. No other files need modification.

---

## Out of Scope

- Laser pointer fade animation
- Eraser precision improvements
- Global hotkey daemon
- Multi-monitor support
- Wayland implementation (stubbed only)
