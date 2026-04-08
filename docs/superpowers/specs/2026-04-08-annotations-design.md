# Annotations — Screen Annotation Tool for Linux
**Date:** 2026-04-08  
**Status:** Approved  
**Stack:** Rust + GTK4 + Cairo

---

## Overview

A native Linux screen annotation tool for real-time drawing and highlighting during screen shares and presentations. More intuitive than Gromit-MPX, with a clean compact floating toolbar inspired by image annotation tools like Shutter. Targets X11 primarily with Wayland best-effort.

---

## Architecture

### Two-window model

The app runs as two GTK4 windows:

1. **Overlay window** — fullscreen, transparent RGBA, always-on-top. This is the drawing canvas. When draw mode is OFF, input regions are cleared so all mouse/keyboard events pass through to the desktop. When draw mode is ON, input is captured and strokes are drawn via Cairo.

2. **Toolbar window** — small floating panel, always-on-top, never transparent. Lives in a corner (default: bottom-left, draggable). Always receives input regardless of draw mode.

### Data model

Strokes are stored as a `Vec<Stroke>` in application state. Each `Stroke` is an enum variant covering the supported tool types. This gives undo (pop last) and clear all (drain vec) for free. On every state change, the overlay redraws from scratch using Cairo.

### Input passthrough

On X11: `gdk_window_input_shape_combine_region()` with an empty region makes the overlay click-through. When draw mode activates, the full window region is restored. GTK4 on Wayland handles this via the same GDK API but delegates to the compositor.

### Shared state

`AppState` is wrapped in `Arc<Mutex<>>` and shared between the overlay and toolbar windows. Contains: current tool, color, stroke width, stroke list, draw mode flag.

### Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Z` | Undo last stroke |
| `Ctrl+Shift+C` / `Escape` | Clear all |
| `Ctrl+H` | Hide/show toolbar |
| `Ctrl+D` | Toggle draw mode |
| `Super+A` | Restore toolbar (configurable, works globally when toolbar is hidden) |

---

## Tools & Drawing Model

### Tool palette

| Tool | Behavior |
|------|----------|
| Pen | Freehand stroke, configurable width |
| Highlighter | Freehand, 50% alpha, thick |
| Line | Click-drag straight line |
| Rectangle | Click-drag, outline only |
| Ellipse | Click-drag, outline only |
| Text | Click to place, type, Enter to commit |
| Laser | Freehand, bright red, fades out after ~1.5s, not stored |
| Eraser | Freehand, removes intersected strokes |

### Stroke data model

```rust
enum Stroke {
    Path   { points: Vec<(f64, f64)>, color: RGBA, width: f64, tool: PathTool },
    Line   { start: (f64, f64), end: (f64, f64), color: RGBA, width: f64 },
    Rect   { origin: (f64, f64), size: (f64, f64), color: RGBA, width: f64 },
    Ellipse{ center: (f64, f64), radii: (f64, f64), color: RGBA, width: f64 },
    Text   { position: (f64, f64), content: String, color: RGBA, size: f64 },
}

enum PathTool { Pen, Highlighter }
```

Laser pointer strokes are rendered live via a timer and are never added to the stroke list.

### Persistence config

`~/.config/annotations/config.toml`:

```toml
[drawing]
stroke_persistence = "permanent"  # or fade_seconds = 5

[toolbar]
position = [40, 40]  # x, y from bottom-left
```

Default: `permanent`.

---

## Toolbar UX

### Layout

Compact horizontal pill-shaped floating bar. Dark semi-transparent background (~90% opacity). Icon-only buttons with tooltips on hover.

```
╭──────────────────────────────────────────────────────────╮
│  ✏  〜  ╱  ▭  ○  T  ●laser  ⌫  │  🎨  ──  │  ↩  ✕  │ ⠿ │
╰──────────────────────────────────────────────────────────╯
```

- **Left group:** tools (mutually exclusive, active tool highlighted)
- **Middle group:** color picker + stroke width
- **Right group:** undo + clear all
- **Far right:** drag handle to reposition

### Active tool feedback

Selected tool button gets a subtle highlight (lighter background or accent ring). Color button fill reflects current color.

### Draw mode indicator

When draw mode is ON, the toolbar gets a thin colored border (red). This is the primary UX signal that clicks will draw rather than pass through.

### Toolbar hide

- `Ctrl+H` hides the toolbar completely
- A configurable hotkey (default: `Super+A`) restores it
- Draw mode state is preserved while toolbar is hidden

### Sizing

- Height: ~40px
- Icon size: 20px  
- Total width: ~420px
- Position persisted to config on move

---

## Project Structure

```
annotations/
├── Cargo.toml
├── src/
│   ├── main.rs        # app entry, GTK init, window setup
│   ├── overlay.rs     # fullscreen transparent canvas window
│   ├── toolbar.rs     # floating toolbar window
│   ├── stroke.rs      # Stroke enum + Cairo rendering
│   ├── state.rs       # shared AppState (Arc<Mutex<>>)
│   ├── input.rs       # input passthrough logic, event handlers
│   └── config.rs      # config.toml read/write
├── assets/
│   └── icons/         # SVG icons for toolbar buttons
└── docs/
    └── superpowers/specs/
```

### Key dependencies

```toml
[dependencies]
gtk4 = "0.9"
cairo-rs = "0.20"
gdk4 = "0.9"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

---

## MVP Scope

### In scope

- All 8 tools (pen, highlighter, line, rect, ellipse, text, laser, eraser)
- Undo last stroke + clear all
- Color picker (6 presets + custom)
- Stroke width (3 sizes: thin/medium/thick)
- Input passthrough toggle (draw mode on/off)
- Toolbar hide/show via shortcut
- Config file (persistence mode, toolbar position)
- X11 primary target, Wayland best-effort

### Out of scope (post-MVP)

- Per-item selection and delete
- Wayland-specific input handling hardening
- Multi-monitor support
- Screenshot/export of annotations
- Global hotkey daemon (shortcuts require toolbar focus in MVP)
