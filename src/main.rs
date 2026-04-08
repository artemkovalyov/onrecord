mod config;
mod input;
mod overlay;
mod state;
mod stroke;
mod toolbar;

use gtk4::prelude::*;
use gtk4::Application;
use glib::ExitCode;

const APP_ID: &str = "dev.annotations.app";

fn main() -> ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    let state = state::new_shared_state();

    app.connect_activate(move |app| {
        let cfg = config::Config::load();
        let overlay_win = overlay::build_overlay(app, state.clone());
        overlay_win.present();

        // Start click-through (passthrough mode, draw mode OFF)
        overlay::set_input_passthrough(&overlay_win, true);

        let overlay_for_redraw = overlay_win.clone();
        let on_redraw = move || {
            overlay::queue_redraw(&overlay_for_redraw);
        };

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
    });

    app.run()
}
