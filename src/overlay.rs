use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};
use std::sync::{Arc, Mutex};
use crate::state::AppState;
use crate::stroke::{render_stroke, render_laser};

pub fn build_overlay(
    app: &Application,
    state: Arc<Mutex<AppState>>,
    monitor: gdk4::Monitor,
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("annotations-overlay")
        .decorated(false)
        .resizable(false)
        .build();

    // Scoped CSS: only makes THIS window transparent, not toolbar menus.
    window.set_widget_name("annotations-overlay");
    let css = gtk4::CssProvider::new();
    css.load_from_string("#annotations-overlay { background: transparent; }");
    gtk4::style_context_add_provider_for_display(
        &gdk4::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );

    // fullscreen_on_monitor: KWin handles geometry correctly; CSS makes it transparent.
    window.fullscreen_on_monitor(&monitor);

    // After present(), send _NET_WM_STATE_ABOVE via idle so KWin has mapped the window.
    window.connect_realize(|win| {
        let win = win.clone();
        glib::idle_add_local_once(move || {
            crate::platform::setup_overlay_pre_map(&win);
        });
    });

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
    window
}

/// Returns the DrawingArea child of the overlay window.
/// Used by main.rs to attach input events after both windows are built.
pub fn drawing_area(window: &ApplicationWindow) -> DrawingArea {
    window.child()
        .and_downcast::<DrawingArea>()
        .expect("overlay child is not a DrawingArea")
}

/// Sets the overlay input region.
///
/// pass_through=true  → empty region (all clicks fall through to windows below)
/// pass_through=false → full monitor minus `toolbar_rect` so toolbar stays clickable
///
/// toolbar_rect: (x, y, width, height) in screen logical coordinates, or None to capture everything
pub fn set_input_passthrough(
    window: &ApplicationWindow,
    pass_through: bool,
    toolbar_rect: Option<(i32, i32, i32, i32)>,
) {
    use gdk4::prelude::SurfaceExt;
    let Some(surface) = window.surface() else { return };
    if pass_through {
        // Empty region = fully click-through
        let region = cairo::Region::create();
        surface.set_input_region(Some(&region));
    } else {
        // Use a very large rectangle to cover the entire screen regardless of scaling.
        // Input regions are in surface (device-independent) coordinates on X11.
        let full = cairo::RectangleInt::new(0, 0, 32767, 32767);
        let region = cairo::Region::create_rectangle(&full);
        if let Some((tx, ty, tw, th)) = toolbar_rect {
            // The overlay is fullscreen at (0,0) so screen coords == surface coords.
            let hole = cairo::RectangleInt::new(tx, ty, tw, th);
            let _ = region.subtract_rectangle(&hole);
        }
        surface.set_input_region(Some(&region));
    }
}

pub fn queue_redraw(window: &ApplicationWindow) {
    if let Some(child) = window.child() {
        if let Ok(da) = child.downcast::<DrawingArea>() {
            da.queue_draw();
        }
    }
}
