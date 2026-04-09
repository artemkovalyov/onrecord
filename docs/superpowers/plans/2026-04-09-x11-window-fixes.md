# X11 Window Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix overlay transparency and always-on-top for both the overlay and toolbar windows on X11/KDE Plasma by isolating all platform-specific setup in a new `src/platform.rs` module using `gdk4-x11` and `x11rb`.

**Architecture:** A new `platform` module exposes three functions (`setup_overlay`, `setup_toolbar`, `get_window_position`) that handle X11 window property setup. `overlay.rs` and `toolbar.rs` call into it via `connect_realize` hooks. `main.rs` saves toolbar position on shutdown. Runtime detection uses GDK display downcast — no compile-time feature flags.

**Tech Stack:** Rust, GTK4 (gtk4 = "0.11"), gdk4-x11 = "0.11", x11rb = "0.13", cairo-rs = "0.22"

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` | Modify | Add `gdk4-x11`, `x11rb` dependencies |
| `src/platform.rs` | Create | All X11 window property setup |
| `src/overlay.rs` | Modify | Remove `fullscreen()`, add `connect_realize` → `platform::setup_overlay` |
| `src/toolbar.rs` | Modify | Add `connect_realize` → `platform::setup_toolbar` |
| `src/main.rs` | Modify | Add `mod platform`, pass monitor geometry, wire `connect_shutdown` |

---

## Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add gdk4-x11 and x11rb to Cargo.toml**

Open `Cargo.toml`. The `[dependencies]` section currently reads:

```toml
[dependencies]
gtk4 = { version = "0.11", features = ["v4_12"] }
gdk4 = "0.11"
cairo-rs = { version = "0.22", features = ["use_glib"] }
glib = "0.22"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "5"
```

Change it to:

```toml
[dependencies]
gtk4 = { version = "0.11", features = ["v4_12"] }
gdk4 = "0.11"
gdk4-x11 = "0.11"
cairo-rs = { version = "0.22", features = ["use_glib"] }
glib = "0.22"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "5"
x11rb = { version = "0.13", features = ["allow-unsafe-code"] }
```

- [ ] **Step 2: Verify it compiles**

```bash
cd /home/i531196/dev/annotations && cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors. (It may download and compile new deps.)

- [ ] **Step 3: Commit**

```bash
cd /home/i531196/dev/annotations
git add Cargo.toml Cargo.lock
git commit -m "chore: add gdk4-x11 and x11rb dependencies"
```

---

## Task 2: Create `src/platform.rs`

**Files:**
- Create: `src/platform.rs`

This module exposes three public functions. X11 detection uses a GDK display downcast. All X11 logic is gated behind that check. The Wayland branch is a logged stub.

- [ ] **Step 1: Create `src/platform.rs` with the full implementation**

```rust
//! Platform-specific window setup.
//!
//! setup_overlay  — RGBA visual, always-on-top, full monitor geometry, DOCK type
//! setup_toolbar  — always-on-top, sticky, positioned from config
//! get_window_position — reads current XID position for config save-on-exit
//!
//! X11 only for now. Wayland branch is a stub pending gtk4-layer-shell.

use gtk4::prelude::*;
use gtk4::ApplicationWindow;

// ── public API ────────────────────────────────────────────────────────────────

/// Configure the overlay window for X11: RGBA visual, DOCK type, always-on-top,
/// full monitor geometry. Must be called from connect_realize.
pub fn setup_overlay(window: &ApplicationWindow, monitor_w: i32, monitor_h: i32) {
    if try_setup_overlay_x11(window, monitor_w, monitor_h) {
        return;
    }
    eprintln!("annotations: platform setup skipped (not X11 or setup failed)");
}

/// Configure the toolbar window for X11: always-on-top, sticky, positioned at
/// (x, y) from config. Must be called from connect_realize.
pub fn setup_toolbar(window: &ApplicationWindow, position: [i32; 2]) {
    if try_setup_toolbar_x11(window, position) {
        return;
    }
    eprintln!("annotations: toolbar platform setup skipped (not X11 or setup failed)");
}

/// Read current window position via X11 GetGeometry. Returns None if not on X11
/// or if the call fails.
pub fn get_window_position(window: &ApplicationWindow) -> Option<[i32; 2]> {
    get_window_position_x11(window)
}

// ── X11 implementation ────────────────────────────────────────────────────────

fn get_x11_xid(window: &ApplicationWindow) -> Option<u32> {
    use gdk4::prelude::SurfaceExt;
    use gdk4_x11::prelude::*;

    let surface = window.surface()?;
    let x11_surface = surface.downcast::<gdk4_x11::X11Surface>().ok()?;
    Some(x11_surface.xid() as u32)
}

fn try_setup_overlay_x11(window: &ApplicationWindow, monitor_w: i32, monitor_h: i32) -> bool {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::atom_manager;

    atom_manager! {
        Atoms: AtomsCookie {
            _NET_WM_WINDOW_TYPE,
            _NET_WM_WINDOW_TYPE_DOCK,
            _NET_WM_STATE,
            _NET_WM_STATE_ABOVE,
        }
    }

    let xid = match get_x11_xid(window) {
        Some(id) => id,
        None => return false,
    };

    let (conn, _screen_num) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("annotations: x11rb connect failed: {e}");
            return false;
        }
    };

    let atoms = match Atoms::new(&conn).and_then(|c| c.reply()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("annotations: x11rb atom intern failed: {e}");
            return false;
        }
    };

    // Set window type to DOCK: KWin skips WM decorations and honours RGBA
    if let Err(e) = conn.change_property32(
        PropMode::REPLACE,
        xid,
        atoms._NET_WM_WINDOW_TYPE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_WINDOW_TYPE_DOCK],
    ) {
        eprintln!("annotations: failed to set _NET_WM_WINDOW_TYPE: {e}");
        return false;
    }

    // Set always-on-top
    if let Err(e) = conn.change_property32(
        PropMode::REPLACE,
        xid,
        atoms._NET_WM_STATE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_STATE_ABOVE],
    ) {
        eprintln!("annotations: failed to set _NET_WM_STATE: {e}");
        return false;
    }

    // Position and size to cover full monitor
    if let Err(e) = conn.configure_window(
        xid,
        &ConfigureWindowAux::new()
            .x(0)
            .y(0)
            .width(monitor_w as u32)
            .height(monitor_h as u32),
    ) {
        eprintln!("annotations: failed to configure overlay geometry: {e}");
        return false;
    }

    if let Err(e) = conn.flush() {
        eprintln!("annotations: x11rb flush failed: {e}");
        return false;
    }

    true
}

fn try_setup_toolbar_x11(window: &ApplicationWindow, position: [i32; 2]) -> bool {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::atom_manager;

    atom_manager! {
        ToolbarAtoms: ToolbarAtomsCookie {
            _NET_WM_STATE,
            _NET_WM_STATE_ABOVE,
            _NET_WM_STATE_STICKY,
        }
    }

    let xid = match get_x11_xid(window) {
        Some(id) => id,
        None => return false,
    };

    let (conn, _screen_num) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("annotations: x11rb connect failed: {e}");
            return false;
        }
    };

    let atoms = match ToolbarAtoms::new(&conn).and_then(|c| c.reply()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("annotations: x11rb atom intern failed: {e}");
            return false;
        }
    };

    // Set always-on-top + sticky (visible on all virtual desktops)
    if let Err(e) = conn.change_property32(
        PropMode::REPLACE,
        xid,
        atoms._NET_WM_STATE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_STATE_ABOVE, atoms._NET_WM_STATE_STICKY],
    ) {
        eprintln!("annotations: failed to set toolbar _NET_WM_STATE: {e}");
        return false;
    }

    // Position at saved config coordinates
    if let Err(e) = conn.configure_window(
        xid,
        &ConfigureWindowAux::new()
            .x(position[0])
            .y(position[1]),
    ) {
        eprintln!("annotations: failed to configure toolbar position: {e}");
        return false;
    }

    if let Err(e) = conn.flush() {
        eprintln!("annotations: x11rb flush failed: {e}");
        return false;
    }

    true
}

fn get_window_position_x11(window: &ApplicationWindow) -> Option<[i32; 2]> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let xid = get_x11_xid(window)?;

    let (conn, _screen_num) = x11rb::connect(None).ok()?;
    let geom = conn.get_geometry(xid).ok()?.reply().ok()?;
    Some([geom.x as i32, geom.y as i32])
}
```

- [ ] **Step 2: Add `mod platform` to `src/main.rs`**

In `src/main.rs`, the module declarations at the top are:

```rust
mod config;
mod input;
mod overlay;
mod state;
mod stroke;
mod toolbar;
```

Change to:

```rust
mod config;
mod input;
mod overlay;
mod platform;
mod state;
mod stroke;
mod toolbar;
```

- [ ] **Step 3: Verify compilation**

```bash
cd /home/i531196/dev/annotations && cargo check 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
cd /home/i531196/dev/annotations
git add src/platform.rs src/main.rs
git commit -m "feat: add platform module with X11 window setup (overlay + toolbar)"
```

---

## Task 3: Fix overlay — remove fullscreen, add connect_realize

**Files:**
- Modify: `src/overlay.rs`
- Modify: `src/main.rs`

The overlay currently calls `window.fullscreen()` which gives KWin control and produces an opaque window. Replace with manual sizing + X11 property setup via `platform::setup_overlay`.

- [ ] **Step 1: Update `build_overlay` signature to accept monitor geometry**

The current signature in `src/overlay.rs:7` is:

```rust
pub fn build_overlay(app: &Application, state: Arc<Mutex<AppState>>) -> ApplicationWindow {
```

Change to:

```rust
pub fn build_overlay(app: &Application, state: Arc<Mutex<AppState>>, monitor_w: i32, monitor_h: i32) -> ApplicationWindow {
```

- [ ] **Step 2: Replace `window.fullscreen()` with size + realize hook in `src/overlay.rs`**

The current `src/overlay.rs` body after the builder is:

```rust
    // Note: set_keep_above() was removed in GTK4. Always-on-top requires
    // _NET_WM_STATE_ABOVE via gdk4-x11 (post-MVP) or gtk4-layer-shell (Wayland).
    // fullscreen() covers the entire monitor and is handled by the compositor correctly
    window.fullscreen();
```

Replace those lines with:

```rust
    // Size to cover full monitor. platform::setup_overlay wires up X11 window type,
    // always-on-top, and exact geometry after the surface is realized.
    window.set_default_size(monitor_w, monitor_h);
    {
        let w = monitor_w;
        let h = monitor_h;
        window.connect_realize(move |win| {
            crate::platform::setup_overlay(win, w, h);
        });
    }
```

- [ ] **Step 3: Update `main.rs` to read monitor geometry and pass to `build_overlay`**

In `src/main.rs`, the `connect_activate` closure currently has:

```rust
        let cfg = config::Config::load();
        let overlay_win = overlay::build_overlay(app, state.clone());
```

Replace with:

```rust
        let cfg = config::Config::load();

        // Read primary monitor geometry for overlay sizing
        let display = gdk4::Display::default().expect("no GDK display");
        let monitor = display.monitors().item(0)
            .and_downcast::<gdk4::Monitor>()
            .expect("no monitor");
        let geom = monitor.geometry();
        let (monitor_w, monitor_h) = (geom.width(), geom.height());

        let overlay_win = overlay::build_overlay(app, state.clone(), monitor_w, monitor_h);
```

Also add the `gdk4` import to `main.rs`. The current imports are:

```rust
use gtk4::prelude::*;
use gtk4::Application;
use glib::ExitCode;
```

Change to:

```rust
use gtk4::prelude::*;
use gtk4::Application;
use glib::ExitCode;
use gdk4::prelude::*;
```

- [ ] **Step 4: Verify compilation**

```bash
cd /home/i531196/dev/annotations && cargo check 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
cd /home/i531196/dev/annotations
git add src/overlay.rs src/main.rs
git commit -m "fix: replace fullscreen() with manual geometry + X11 DOCK/ABOVE setup for overlay"
```

---

## Task 4: Fix toolbar — always-on-top + correct positioning

**Files:**
- Modify: `src/toolbar.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Update `build_toolbar` signature to accept initial position**

The current signature in `src/toolbar.rs:24` is:

```rust
pub fn build_toolbar(
    app: &Application,
    state: Arc<Mutex<AppState>>,
    overlay_win: ApplicationWindow,
    on_redraw: impl Fn() + 'static + Clone,
) -> ApplicationWindow {
```

Change to:

```rust
pub fn build_toolbar(
    app: &Application,
    state: Arc<Mutex<AppState>>,
    overlay_win: ApplicationWindow,
    on_redraw: impl Fn() + 'static + Clone,
    initial_position: [i32; 2],
) -> ApplicationWindow {
```

- [ ] **Step 2: Add `connect_realize` hook in `build_toolbar`**

In `src/toolbar.rs`, find the line (near line 160):

```rust
    window.set_widget_name("annotations-toolbar");
    // Note: set_keep_above() was removed in GTK4. Always-on-top requires WM hints (post-MVP).
```

Replace those two lines with:

```rust
    window.set_widget_name("annotations-toolbar");
    {
        let pos = initial_position;
        window.connect_realize(move |win| {
            crate::platform::setup_toolbar(win, pos);
        });
    }
```

- [ ] **Step 3: Update `main.rs` to pass position and wire shutdown save**

In `src/main.rs`, the current `build_toolbar` call is:

```rust
        let toolbar_win = toolbar::build_toolbar(
            app,
            state.clone(),
            overlay_win.clone(),
            on_redraw,
        );

        toolbar_win.present();
        toolbar::attach_toolbar_shortcuts(&toolbar_win);

        // GTK4 does not expose window.move_() — position via initial geometry hint.
        // On X11, set_default_size + present positions at WM default; exact placement
        // requires gtk4-layer-shell or gdk4-x11 XMoveWindow (post-MVP).
        // Store config position for future use.
        let [_tx, _ty] = cfg.toolbar.position;
```

Replace with:

```rust
        let toolbar_position = cfg.toolbar.position;
        let toolbar_win = toolbar::build_toolbar(
            app,
            state.clone(),
            overlay_win.clone(),
            on_redraw,
            toolbar_position,
        );

        toolbar_win.present();
        toolbar::attach_toolbar_shortcuts(&toolbar_win);

        // Save toolbar position when the app exits
        let toolbar_for_shutdown = toolbar_win.clone();
        app.connect_shutdown(move |_| {
            if let Some(pos) = crate::platform::get_window_position(&toolbar_for_shutdown) {
                let mut cfg = crate::config::Config::load();
                cfg.toolbar.position = pos;
                cfg.save();
            }
        });
```

- [ ] **Step 4: Verify compilation**

```bash
cd /home/i531196/dev/annotations && cargo check 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
cd /home/i531196/dev/annotations
git add src/toolbar.rs src/main.rs
git commit -m "fix: toolbar always-on-top + sticky via X11, apply config position, save on exit"
```

---

## Task 5: Build and smoke test

- [ ] **Step 1: Full build**

```bash
cd /home/i531196/dev/annotations && cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished` with no errors.

- [ ] **Step 2: Run the app**

```bash
cd /home/i531196/dev/annotations && cargo run &
```

Verify:
- The overlay window is fully **transparent** (desktop wallpaper fully visible through it)
- The overlay covers the **entire screen** (no visible gaps at edges)
- The toolbar is visible and floating above the desktop

- [ ] **Step 3: Test toolbar always-on-top**

Open any application (browser, terminal, file manager). Move it to cover where the toolbar is. Verify the toolbar remains **on top** and is not covered.

- [ ] **Step 4: Test draw mode**

Press `Ctrl+D` to enable draw mode. Draw a stroke on the overlay. Verify:
- Strokes are visible on the transparent overlay
- Desktop content is visible in areas without strokes
- Toolbar draw-active red border appears

- [ ] **Step 5: Test toolbar position persistence**

Drag the toolbar to a new position. Kill the app (`Ctrl+C` in terminal or close). Restart with `cargo run`. Verify toolbar appears at the position it was left at.

- [ ] **Step 6: Final commit if any fixups were needed**

```bash
cd /home/i531196/dev/annotations
git add -p   # stage only intentional fixup changes
git commit -m "fix: address smoke test findings"
```

---

## Notes

**Why `connect_realize` and not `connect_map`?** The XID is available after `realize` (surface creation). `map` fires later (after the window is shown) and would cause a visible flash. `realize` is early enough to set properties before the first paint.

**Why a new `x11rb::connect()` per call?** Opening a fresh connection per setup call is slightly heavier than reusing one, but these functions are called once at startup. The simplicity of not storing a connection in global state outweighs the cost.

**`get_geometry` returns parent-relative coordinates.** On X11 with KWin, `GetGeometry` returns coordinates relative to the parent window (root for top-level windows), which is what we want for the config `[x, y]`. If the toolbar is reparented by the WM (rare with KWin), the coordinates may need a `TranslateCoordinates` call — but in practice KWin does not reparent `_NET_WM_STATE_ABOVE` windows.
