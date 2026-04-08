use gtk4::prelude::*;
use gtk4::{ApplicationWindow, DrawingArea, EventControllerKey, GestureClick, GestureDrag};
use gdk4::ModifierType;
use glib::clone;
use std::sync::{Arc, Mutex};
use crate::state::{AppState, PathTool, Stroke, StrokeWidth, Tool};
use crate::overlay::set_input_passthrough;

pub fn attach_drawing_events(
    overlay: &ApplicationWindow,
    drawing_area: &DrawingArea,
    state: Arc<Mutex<AppState>>,
) {
    // -- Drag gesture (freehand, line, rect, ellipse, laser, eraser) --
    let drag = GestureDrag::new();
    drag.set_button(gdk4::BUTTON_PRIMARY);

    let state_begin = state.clone();
    drag.connect_drag_begin(clone!(#[weak] drawing_area, move |_gesture, x, y| {
        let st = state_begin.lock().unwrap();
        if !st.draw_mode { return; }
        let color = st.active_color;
        let width = st.stroke_width.pixels();
        let tool = st.active_tool;
        drop(st);

        let stroke = match tool {
            Tool::Pen => Stroke::Path {
                points: vec![(x, y)],
                color,
                width,
                tool: PathTool::Pen,
            },
            Tool::Highlighter => Stroke::Path {
                points: vec![(x, y)],
                color,
                width: StrokeWidth::Thick.pixels(),
                tool: PathTool::Highlighter,
            },
            Tool::Line => Stroke::Line { start: (x, y), end: (x, y), color, width },
            Tool::Rectangle => Stroke::Rect { origin: (x, y), size: (0.0, 0.0), color, width },
            Tool::Ellipse => Stroke::Ellipse { center: (x, y), radii: (0.0, 0.0), color, width },
            Tool::Laser => {
                state_begin.lock().unwrap().laser_points = vec![(x, y)];
                drawing_area.queue_draw();
                return;
            }
            Tool::Eraser | Tool::Text => return,
        };
        state_begin.lock().unwrap().current_stroke = Some(stroke);
        drawing_area.queue_draw();
    }));

    let state_update = state.clone();
    drag.connect_drag_update(clone!(#[weak] drawing_area, move |gesture, ox, oy| {
        let (sx, sy) = gesture.start_point().unwrap_or((0.0, 0.0));
        let (x, y) = (sx + ox, sy + oy);
        let mut st = state_update.lock().unwrap();
        if !st.draw_mode { return; }

        match st.active_tool {
            Tool::Laser => {
                st.laser_points.push((x, y));
            }
            Tool::Pen | Tool::Highlighter => {
                if let Some(Stroke::Path { ref mut points, .. }) = st.current_stroke {
                    points.push((x, y));
                }
            }
            Tool::Line => {
                if let Some(Stroke::Line { ref mut end, .. }) = st.current_stroke {
                    *end = (x, y);
                }
            }
            Tool::Rectangle => {
                if let Some(Stroke::Rect { origin, ref mut size, .. }) = st.current_stroke {
                    *size = (x - origin.0, y - origin.1);
                }
            }
            Tool::Ellipse => {
                if let Some(Stroke::Ellipse { center, ref mut radii, .. }) = st.current_stroke {
                    *radii = ((x - center.0).abs(), (y - center.1).abs());
                }
            }
            _ => {}
        }
        drop(st);
        drawing_area.queue_draw();
    }));

    let state_end = state.clone();
    drag.connect_drag_end(clone!(#[weak] drawing_area, move |gesture, ox, oy| {
        let (sx, sy) = gesture.start_point().unwrap_or((0.0, 0.0));
        let (x, y) = (sx + ox, sy + oy);
        let mut st = state_end.lock().unwrap();
        if !st.draw_mode { return; }

        match st.active_tool {
            Tool::Laser => {
                st.laser_points.clear();
            }
            Tool::Eraser => {
                st.strokes.retain(|s| !stroke_hit_test(s, sx, sy, x, y));
            }
            Tool::Text => {
                // Text strokes are committed only via Enter key — not on drag end
            }
            _ => {
                if let Some(finished) = st.current_stroke.take() {
                    st.strokes.push(finished);
                }
            }
        }
        st.current_stroke = None;
        drop(st);
        drawing_area.queue_draw();
    }));

    drawing_area.add_controller(drag);

    // -- Click gesture (text tool placement) --
    let click = GestureClick::new();
    click.set_button(gdk4::BUTTON_PRIMARY);
    let state_click = state.clone();
    click.connect_pressed(move |_, _, x, y| {
        let mut st = state_click.lock().unwrap();
        if !st.draw_mode || st.active_tool != Tool::Text { return; }
        // Commit any pending non-empty text before starting a new one
        if let Some(Stroke::Text { ref content, .. }) = st.current_stroke {
            if !content.is_empty() {
                if let Some(finished) = st.current_stroke.take() {
                    st.strokes.push(finished);
                }
            }
        }
        let color = st.active_color;
        st.current_stroke = Some(Stroke::Text {
            position: (x, y),
            content: String::new(),
            color,
            size: 24.0,
        });
    });
    drawing_area.add_controller(click);

    // -- Keyboard (shortcuts + text input) --
    let key_ctrl = EventControllerKey::new();
    let state_key = state.clone();
    let overlay_key = overlay.clone();
    let da_key = drawing_area.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, modifiers| {
        let ctrl = modifiers.contains(ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(ModifierType::SHIFT_MASK);
        use gdk4::Key;

        // Text input mode
        {
            let mut st = state_key.lock().unwrap();
            if st.draw_mode && st.active_tool == Tool::Text {
                if let Some(Stroke::Text { ref mut content, .. }) = st.current_stroke {
                    match key {
                        Key::Return | Key::KP_Enter => {
                            // Commit text stroke — take() is safe here because the if-let above confirms it's Some
                            if let Some(finished) = st.current_stroke.take() {
                                st.strokes.push(finished);
                            }
                            drop(st);
                            da_key.queue_draw();
                            return glib::Propagation::Stop;
                        }
                        Key::BackSpace => {
                            content.pop();
                            drop(st);
                            da_key.queue_draw();
                            return glib::Propagation::Stop;
                        }
                        _ => {
                            if let Some(ch) = key.to_unicode() {
                                if !ch.is_control() {
                                    content.push(ch);
                                    drop(st);
                                    da_key.queue_draw();
                                    return glib::Propagation::Stop;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Global shortcuts
        match key {
            Key::z if ctrl && !shift => {
                state_key.lock().unwrap().undo();
                da_key.queue_draw();
                glib::Propagation::Stop
            }
            Key::c if ctrl && shift => {
                state_key.lock().unwrap().clear();
                da_key.queue_draw();
                glib::Propagation::Stop
            }
            Key::Escape => {
                state_key.lock().unwrap().clear();
                da_key.queue_draw();
                glib::Propagation::Stop
            }
            Key::d if ctrl => {
                let mut st = state_key.lock().unwrap();
                st.draw_mode = !st.draw_mode;
                let dm = st.draw_mode;
                drop(st);
                set_input_passthrough(&overlay_key, !dm);
                da_key.queue_draw();
                glib::Propagation::Stop
            }
            // Ctrl+H: pass through so toolbar's own key controller can handle hide/show
            Key::h if ctrl => glib::Propagation::Proceed,
            _ => glib::Propagation::Proceed,
        }
    });
    overlay.add_controller(key_ctrl);
}

/// Hit-test for eraser: checks if the drag path midpoint is near a stroke.
fn stroke_hit_test(stroke: &Stroke, x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
    let threshold = 20.0_f64;
    match stroke {
        Stroke::Path { points, .. } => {
            points.iter().any(|(px, py)| (px - mx).hypot(py - my) < threshold)
        }
        Stroke::Line { start, end, .. } => {
            (start.0 - mx).hypot(start.1 - my) < threshold
                || (end.0 - mx).hypot(end.1 - my) < threshold
        }
        Stroke::Rect { origin, size, .. } => {
            let (x0, x1) = if size.0 >= 0.0 { (origin.0, origin.0 + size.0) } else { (origin.0 + size.0, origin.0) };
            let (y0, y1) = if size.1 >= 0.0 { (origin.1, origin.1 + size.1) } else { (origin.1 + size.1, origin.1) };
            mx >= x0 && mx <= x1 && my >= y0 && my <= y1
        }
        Stroke::Ellipse { center, radii, .. } => {
            let dx = (mx - center.0) / (radii.0 + threshold);
            let dy = (my - center.1) / (radii.1 + threshold);
            dx * dx + dy * dy <= 1.0
        }
        Stroke::Text { position, .. } => {
            (position.0 - mx).hypot(position.1 - my) < threshold * 2.0
        }
    }
}
