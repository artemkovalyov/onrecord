use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};
use std::sync::{Arc, Mutex};
use crate::state::AppState;
use crate::stroke::{render_stroke, render_laser};

pub fn build_overlay(app: &Application, state: Arc<Mutex<AppState>>) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("annotations-overlay")
        .decorated(false)
        .resizable(false)
        .build();

    // Note: set_keep_above() was removed in GTK4. Always-on-top requires
    // _NET_WM_STATE_ABOVE via gdk4-x11 (post-MVP) or gtk4-layer-shell (Wayland).
    // fullscreen() covers the entire monitor and is handled by the compositor correctly
    window.fullscreen();

    let drawing_area = DrawingArea::new();
    drawing_area.set_hexpand(true);
    drawing_area.set_vexpand(true);

    let state_draw = state.clone();
    drawing_area.set_draw_func(move |_da, cr, _w, _h| {
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = cr.paint();
        cr.set_operator(cairo::Operator::Over);

        let st = state_draw.lock().unwrap();
        for stroke in &st.strokes {
            render_stroke(cr, stroke);
        }
        if let Some(ref s) = st.current_stroke {
            render_stroke(cr, s);
        }
        if !st.laser_points.is_empty() {
            render_laser(cr, &st.laser_points, 0.9);
        }
    });

    window.set_child(Some(&drawing_area));

    // Attach drawing event handlers (defined in input.rs)
    crate::input::attach_drawing_events(&window, &drawing_area, state);

    window
}

pub fn set_input_passthrough(window: &ApplicationWindow, pass_through: bool) {
    use gdk4::prelude::SurfaceExt;
    // The window is always realized when this is called (after present()).
    // Do NOT register connect_realize here — that would accumulate handlers on every toggle.
    if let Some(surface) = window.surface() {
        if pass_through {
            let region = cairo::Region::create();
            surface.set_input_region(Some(&region));
        } else {
            surface.set_input_region(None::<&cairo::Region>);
        }
    }
}

pub fn queue_redraw(window: &ApplicationWindow) {
    if let Some(child) = window.child() {
        if let Ok(da) = child.downcast::<DrawingArea>() {
            da.queue_draw();
        }
    }
}
