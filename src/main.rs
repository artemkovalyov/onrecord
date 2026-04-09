mod config;
mod input;
mod overlay;
mod platform;
mod state;
mod stroke;
mod toolbar;

use gtk4::prelude::*;
use gtk4::Application;
use glib::ExitCode;
use gdk4::prelude::*;

const APP_ID: &str = "dev.annotations.app";

fn main() -> ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    let state = state::new_shared_state();

    app.connect_activate(move |app| {
        let cfg = config::Config::load();

        let display = gdk4::Display::default().expect("no GDK display");
        let monitor = display.monitors().item(0)
            .and_downcast::<gdk4::Monitor>()
            .expect("no monitor");

        // Build overlay window (CSS + fullscreen_on_monitor, no input events yet)
        let overlay_win = overlay::build_overlay(app, state.clone(), monitor);
        overlay_win.present();
        overlay::set_input_passthrough(&overlay_win, true, None);

        let overlay_for_redraw = overlay_win.clone();
        let on_redraw = move || {
            overlay::queue_redraw(&overlay_for_redraw);
        };

        // Build toolbar window
        let toolbar_win = toolbar::build_toolbar(
            app,
            state.clone(),
            overlay_win.clone(),
            on_redraw,
            cfg.toolbar.position,
        );
        toolbar_win.present();
        toolbar::attach_toolbar_shortcuts(&toolbar_win);

        // Attach drawing events AFTER toolbar is built, so Ctrl+D can update toolbar CSS.
        // on_draw_mode_change: called when Ctrl+D toggles draw mode via keyboard shortcut.
        {
            let tw = toolbar_win.clone();
            let ow = overlay_win.clone();
            let on_draw_mode_change = move |draw_mode: bool| {
                if draw_mode {
                    tw.add_css_class("draw-active");
                } else {
                    tw.remove_css_class("draw-active");
                }
                let toolbar_rect = if draw_mode {
                    crate::platform::get_window_position(&tw).map(|[x, y]| {
                        let (w, h) = (tw.width(), tw.height());
                        (x, y, w, h)
                    })
                } else {
                    None
                };
                overlay::set_input_passthrough(&ow, !draw_mode, toolbar_rect);
            };
            input::attach_drawing_events(
                &overlay_win,
                &overlay::drawing_area(&overlay_win),
                state.clone(),
                on_draw_mode_change,
            );
        }

        // Save toolbar position on exit
        let toolbar_for_shutdown = toolbar_win.clone();
        app.connect_shutdown(move |_| {
            if let Some(pos) = platform::get_window_position(&toolbar_for_shutdown) {
                let mut cfg = config::Config::load();
                cfg.toolbar.position = pos;
                cfg.save();
            }
        });
    });

    app.run()
}
