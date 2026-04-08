// ColorButton deprecated since GTK 4.10; ColorDialog replacement deferred to post-MVP
#![allow(deprecated)]

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, ColorButton,
    EventControllerKey, Orientation, Separator,
};
use gdk4::{Key, ModifierType, RGBA};
use std::sync::{Arc, Mutex};
use crate::state::{AppState, Color, StrokeWidth, Tool};

fn tool_button(label: &str, tooltip: &str) -> Button {
    let btn = Button::with_label(label);
    btn.set_tooltip_text(Some(tooltip));
    btn.set_size_request(36, 36);
    btn
}

fn separator() -> Separator {
    Separator::new(Orientation::Vertical)
}

pub fn build_toolbar(
    app: &Application,
    state: Arc<Mutex<AppState>>,
    overlay_win: ApplicationWindow,
    on_redraw: impl Fn() + 'static + Clone,
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("annotations-toolbar")
        .decorated(false)
        .resizable(false)
        .build();

    window.set_default_size(440, 48);

    let hbox = GtkBox::new(Orientation::Horizontal, 2);
    hbox.set_margin_start(6);
    hbox.set_margin_end(6);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);

    // -- Tool buttons --
    let tools: &[(&str, &str, Tool)] = &[
        ("\u{270F}", "Pen",         Tool::Pen),
        ("\u{301C}", "Highlighter", Tool::Highlighter),
        ("\u{2571}", "Line",        Tool::Line),
        ("\u{25AD}", "Rectangle",   Tool::Rectangle),
        ("\u{25CB}", "Ellipse",     Tool::Ellipse),
        ("T", "Text",        Tool::Text),
        ("\u{2B24}", "Laser",       Tool::Laser),
        ("\u{232B}", "Eraser",      Tool::Eraser),
    ];

    for (icon, tooltip, tool) in tools {
        let btn = tool_button(icon, tooltip);
        let state_c = state.clone();
        let t = *tool;
        let redraw = on_redraw.clone();
        btn.connect_clicked(move |_| {
            state_c.lock().unwrap().active_tool = t;
            redraw();
        });
        hbox.append(&btn);
    }

    hbox.append(&separator());

    // -- Color button --
    let color_btn = ColorButton::new();
    color_btn.set_tooltip_text(Some("Color"));
    color_btn.set_size_request(36, 36);
    {
        let st = state.lock().unwrap();
        let c = &st.active_color;
        color_btn.set_rgba(&RGBA::new(c.r as f32, c.g as f32, c.b as f32, c.a as f32));
    }
    let state_c = state.clone();
    let redraw = on_redraw.clone();
    color_btn.connect_color_set(move |btn| {
        let rgba = btn.rgba();
        state_c.lock().unwrap().active_color = Color::new(
            rgba.red() as f64,
            rgba.green() as f64,
            rgba.blue() as f64,
            rgba.alpha() as f64,
        );
        redraw();
    });
    hbox.append(&color_btn);

    // -- Width buttons --
    let widths: &[(&str, &str, StrokeWidth)] = &[
        ("\u{2500}", "Thin",   StrokeWidth::Thin),
        ("\u{2501}", "Medium", StrokeWidth::Medium),
        ("\u{25AC}", "Thick",  StrokeWidth::Thick),
    ];
    for (icon, tooltip, width) in widths {
        let btn = tool_button(icon, tooltip);
        let state_c = state.clone();
        let w = *width;
        let redraw = on_redraw.clone();
        btn.connect_clicked(move |_| {
            state_c.lock().unwrap().stroke_width = w;
            redraw();
        });
        hbox.append(&btn);
    }

    hbox.append(&separator());

    // -- Draw mode toggle --
    let draw_btn = Button::with_label("\u{25CF}");
    draw_btn.set_tooltip_text(Some("Toggle draw mode (Ctrl+D)"));
    draw_btn.set_size_request(36, 36);
    let state_c = state.clone();
    let overlay_c = overlay_win.clone();
    let redraw = on_redraw.clone();
    let window_ref = window.clone();
    draw_btn.connect_clicked(move |_btn| {
        let mut st = state_c.lock().unwrap();
        st.draw_mode = !st.draw_mode;
        let dm = st.draw_mode;
        drop(st);
        crate::overlay::set_input_passthrough(&overlay_c, !dm);
        if dm {
            window_ref.add_css_class("draw-active");
        } else {
            window_ref.remove_css_class("draw-active");
        }
        redraw();
    });
    hbox.append(&draw_btn);

    hbox.append(&separator());

    // -- Undo / Clear --
    let undo_btn = tool_button("\u{21A9}", "Undo (Ctrl+Z)");
    let state_c = state.clone();
    let redraw = on_redraw.clone();
    undo_btn.connect_clicked(move |_| {
        state_c.lock().unwrap().undo();
        redraw();
    });
    hbox.append(&undo_btn);

    let clear_btn = tool_button("\u{2715}", "Clear all (Ctrl+Shift+C)");
    let state_c = state.clone();
    let redraw = on_redraw.clone();
    clear_btn.connect_clicked(move |_| {
        state_c.lock().unwrap().clear();
        redraw();
    });
    hbox.append(&clear_btn);

    window.set_child(Some(&hbox));
    window.set_widget_name("annotations-toolbar");
    // Note: set_keep_above() was removed in GTK4. Always-on-top requires WM hints (post-MVP).

    // Apply CSS scoped to toolbar window only (avoids affecting overlay transparency)
    let css = gtk4::CssProvider::new();
    css.load_from_string(
        "#annotations-toolbar { background-color: rgba(30,30,30,0.92); border-radius: 12px; }
         #annotations-toolbar button { background: none; border: none; color: #eee; font-size: 16px; border-radius: 6px; }
         #annotations-toolbar button:hover { background-color: rgba(255,255,255,0.12); }
         #annotations-toolbar.draw-active { border: 2px solid #e53935; }",
    );
    gtk4::style_context_add_provider_for_display(
        &gdk4::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    window
}

/// Note: GTK4 removed `get_position()` — window position is managed by the compositor.
/// For X11, exact positioning requires `gdk4-x11` XMoveWindow (post-MVP).
/// Toolbar position from `~/.config/annotations/config.toml` is loaded on startup
/// and will be applied when X11 positioning is implemented.
pub fn attach_toolbar_shortcuts(window: &ApplicationWindow) {
    let key_ctrl = EventControllerKey::new();
    let win = window.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, modifiers| {
        let ctrl = modifiers.contains(ModifierType::CONTROL_MASK);
        if key == Key::h && ctrl {
            if win.is_visible() {
                win.set_visible(false);
            } else {
                win.set_visible(true);
                win.present();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_ctrl);
}
