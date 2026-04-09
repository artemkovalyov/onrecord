// ColorButton deprecated since GTK 4.10; ColorDialog replacement deferred to post-MVP
#![allow(deprecated)]

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, ColorButton,
    EventControllerKey, GestureClick, Label, Orientation, Separator,
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
    initial_position: [i32; 2],
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("annotations-toolbar")
        .decorated(false)
        .resizable(false)
        .build();

    window.set_default_size(480, 48);

    let hbox = GtkBox::new(Orientation::Horizontal, 2);
    hbox.set_margin_start(4);
    hbox.set_margin_end(6);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);

    // -- Drag handle --
    let drag_handle = Label::new(Some("⠿"));
    drag_handle.set_tooltip_text(Some("Drag to move toolbar"));
    drag_handle.set_margin_start(2);
    drag_handle.set_margin_end(4);
    drag_handle.add_css_class("drag-handle");
    let drag_gesture = GestureClick::new();
    drag_gesture.set_button(gdk4::BUTTON_PRIMARY);
    {
        let win_drag = window.clone();
        drag_gesture.connect_pressed(move |gesture, _n, x, y| {
            use gdk4::prelude::ToplevelExt;
            let Some(surface) = win_drag.surface() else { return };
            let Ok(toplevel) = surface.downcast::<gdk4::Toplevel>() else { return };
            let Some(device) = gesture.device() else { return };
            let ts = gesture.current_event_time();
            toplevel.begin_move(&device, gdk4::BUTTON_PRIMARY as i32, x, y, ts);
        });
    }
    drag_handle.add_controller(drag_gesture);
    hbox.append(&drag_handle);

    hbox.append(&separator());

    // -- Tool buttons --
    // Store (button, tool) pairs so we can update .active CSS class on selection
    let tools: &[(&str, &str, Tool)] = &[
        ("\u{270F}", "Pen",         Tool::Pen),
        ("\u{301C}", "Highlighter", Tool::Highlighter),
        ("\u{2571}", "Line",        Tool::Line),
        ("\u{25AD}", "Rectangle",   Tool::Rectangle),
        ("\u{25CB}", "Ellipse",     Tool::Ellipse),
        ("T",        "Text",        Tool::Text),
        ("\u{2B24}", "Laser",       Tool::Laser),
        ("\u{232B}", "Eraser",      Tool::Eraser),
    ];

    // Collect buttons so we can update their active state
    let tool_buttons: Vec<(Button, Tool)> = tools.iter().map(|(icon, tooltip, tool)| {
        let btn = tool_button(icon, tooltip);
        (*tool == Tool::Pen).then(|| btn.add_css_class("active")); // Pen is default
        (btn, *tool)
    }).collect();

    // Wrap in Arc so each click handler can update all buttons
    let tool_buttons = Arc::new(tool_buttons);

    for (btn, t) in tool_buttons.iter() {
        let state_c = state.clone();
        let redraw = on_redraw.clone();
        let tool = *t;
        let btns = tool_buttons.clone();
        btn.connect_clicked(move |_| {
            state_c.lock().unwrap().active_tool = tool;
            // Update .active class on all tool buttons
            for (b, bt) in btns.iter() {
                if *bt == tool {
                    b.add_css_class("active");
                } else {
                    b.remove_css_class("active");
                }
            }
            redraw();
        });
        hbox.append(btn);
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
        let toolbar_rect = if dm {
            crate::platform::get_window_position(&window_ref).map(|[x, y]| {
                (x, y, window_ref.width(), window_ref.height())
            })
        } else {
            None
        };
        crate::overlay::set_input_passthrough(&overlay_c, !dm, toolbar_rect);
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

    hbox.append(&separator());

    // -- Close button --
    let close_btn = tool_button("\u{23FB}", "Quit (close app)");
    close_btn.add_css_class("close-btn");
    {
        let app_ref = app.clone();
        close_btn.connect_clicked(move |_| {
            app_ref.quit();
        });
    }
    hbox.append(&close_btn);

    window.set_child(Some(&hbox));
    window.set_widget_name("annotations-toolbar");
    // Set ABOVE+STICKY before map
    window.connect_realize(move |win| {
        crate::platform::setup_toolbar_pre_map(win);
    });
    // Reposition and reinforce ABOVE state after map — once only
    {
        let pos = initial_position;
        let positioned = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        window.connect_map(move |win| {
            if positioned.compare_exchange(
                false, true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            ).is_ok() {
                crate::platform::setup_toolbar_post_map(win, pos);
            }
            // Reinforce ABOVE every time the toolbar is shown (handles re-show after hide)
            let win2 = win.clone();
            glib::idle_add_local_once(move || {
                crate::platform::setup_toolbar_above_post_map(&win2);
            });
        });
    }

    // Apply CSS scoped to toolbar window only (avoids affecting overlay transparency)
    let css = gtk4::CssProvider::new();
    css.load_from_string(
        "#annotations-toolbar { background-color: rgba(30,30,30,0.92); border-radius: 12px; }
         #annotations-toolbar button { background: none; border: none; color: #eee; font-size: 16px; border-radius: 6px; }
         #annotations-toolbar button:hover { background-color: rgba(255,255,255,0.12); }
         #annotations-toolbar button.active { background-color: rgba(255,255,255,0.22); box-shadow: inset 0 0 0 1px rgba(255,255,255,0.4); }
         #annotations-toolbar.draw-active { border: 2px solid #e53935; }
         #annotations-toolbar .drag-handle { color: #888; font-size: 18px; cursor: move; padding: 0 4px; }",
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
